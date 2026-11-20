# Frequently Asked Questions

Answers to common integration and usage questions for the Soroban Vesting
Platform.  All answers are grounded in the contract source code — see the
referenced files for the authoritative implementation.

---

## Table of Contents

1. [How does the cliff + linear schedule work?](#1-how-does-the-cliff--linear-schedule-work)
2. [What happens to rounding dust (integer division in vested\_amount)?](#2-what-happens-to-rounding-dust-integer-division-in-vested_amount)
3. [Can the admin revoke a schedule while the contract is paused?](#3-can-the-admin-revoke-a-schedule-while-the-contract-is-paused)
4. [Why is revoke() not pause-gated?](#4-why-is-revoke-not-pause-gated)
5. [Why is execute() permissionless in the multisig?](#5-why-is-execute-permissionless-in-the-multisig)
6. [How do I use the multisig as the vesting admin?](#6-how-do-i-use-the-multisig-as-the-vesting-admin)
7. [What is the difference between Completed and Revoked status?](#7-what-is-the-difference-between-completed-and-revoked-status)
8. [What does extend\_ttl() do and when should I call it?](#8-what-does-extend_ttl-do-and-when-should-i-call-it)
9. [How do I restore an archived contract entry?](#9-how-do-i-restore-an-archived-contract-entry)
10. [Is this safe to use on mainnet?](#10-is-this-safe-to-use-on-mainnet)
11. [What does get\_claimable() return for Completed or Revoked schedules?](#11-what-does-get_claimable-return-for-completed-or-revoked-schedules)
12. [How do owner-set changes (add/remove/threshold) work in the multisig?](#12-how-do-owner-set-changes-addremovethreshold-work-in-the-multisig)

---

## 1. How does the cliff + linear schedule work?

A vesting schedule has three time parameters:

| Parameter | Meaning |
|-----------|---------|
| `start_time` | Unix timestamp when the schedule begins |
| `cliff_duration` | Seconds after `start_time` before any tokens become claimable |
| `total_duration` | Total seconds over which tokens vest linearly (from `start_time`) |

Before `start_time + cliff_duration`, `get_claimable()` always returns `0`.
Once the cliff has elapsed, vesting is computed linearly from `start_time`
(not from the cliff end), so the beneficiary can immediately claim the portion
that vested during the cliff period:

```
vested_amount = elapsed * total_amount / total_duration
```

where `elapsed = now - start_time`, clamped to `[0, total_duration]`.

**Example:** `total_amount = 12 000`, `cliff_duration = 3 months`,
`total_duration = 12 months`.  At the cliff end, `elapsed = 3 months`, so
`3/12 * 12 000 = 3 000` tokens are immediately claimable.

Source: `vested_amount()` in `contracts/vesting/src/lib.rs`.

---

## 2. What happens to rounding dust (integer division in vested\_amount)?

The formula uses integer (floor) division:

```rust
i128::from(elapsed).saturating_mul(schedule.total_amount)
    / i128::from(schedule.total_duration)
```

Any fractional remainder is truncated and stays locked in the vesting contract.
This "dust" accumulates over time and is released only when
`elapsed >= total_duration`, at which point the branch
`return schedule.total_amount` returns the full amount regardless of prior
rounding losses.

**Practical consequence:** With `total_amount = 1` and `total_duration = 100`,
`get_claimable()` returns `0` for every timestamp before the end, and `1`
exactly at `elapsed >= 100`.  This is correct and tested in
`test_create_schedule_minimum_amount`.

No tokens are permanently lost.  The total amount is always conserved:

```
vested + unvested + dust == total_amount
```

where `dust` is the accumulated rounding error that will be released at the end.

Source: `vested_amount()` comment in `contracts/vesting/src/lib.rs`.

---

## 3. Can the admin revoke a schedule while the contract is paused?

Yes.  `revoke()` is deliberately **not** blocked by the pause flag.

When a contract is paused, `create_schedule()` and `claim()` are gated, but
`revoke()` remains fully available.  This means the admin can still recover
unvested tokens and pay out already-vested tokens to the beneficiary during
an emergency freeze.

Without this, a paused contract would permanently lock all deposited tokens
with no recovery path.

Source: `revoke()` implementation and the **Pause semantics** section in
`README.md`; `contracts/vesting/src/lib.rs`.

---

## 4. Why is revoke() not pause-gated?

The design rationale is safety-first: a paused contract must never become a
**permanent fund lock**.

`pause()` is an emergency brake intended to halt new value flows while an
incident is investigated.  However, an admin must still be able to:

1. **Return unvested tokens** to the treasury so locked capital is not stuck.
2. **Pay out already-vested tokens** to beneficiaries who have earned them,
   avoiding liability.

If `revoke()` were pause-gated, those two operations would be blocked.  The
only way to unblock them would be to unpause, which defeats the purpose of
the emergency freeze.

The same logic applies to `transfer_admin()`: blocking admin hand-off could
leave the contract with no way to unpause it at all.

Source: **Pause semantics** table in `README.md`.

---

## 5. Why is execute() permissionless in the multisig?

`execute()` can be called by anyone once the confirmation threshold is met —
it is not restricted to owners.

The rationale: owners expressed their consent by placing on-chain
confirmations.  The consent is already recorded in contract storage.
Requiring an additional auth check on `execute()` would not improve security —
any owner could call it — but it would break the common automation pattern
where an off-chain bot polls for executable proposals and triggers them.

This is a deliberate design choice: the threshold represents the collective
decision; the trigger is a formality.

Source: `execute()` doc comment in `contracts/multisig/src/lib.rs` and the
**Multisig as vesting admin** section in `README.md`.

---

## 6. How do I use the multisig as the vesting admin?

1. **Deploy multisig** with your owner list and threshold.
2. **Deploy vesting** with the multisig contract address as `admin`:
   ```bash
   stellar contract deploy \
     --wasm target/wasm32v1-none/release/vesting.wasm \
     --source my-key --network testnet \
     -- --admin "$MULTISIG_CONTRACT_ID"
   ```
3. To call any admin-only vesting function (e.g. `create_schedule`, `pause`,
   `revoke`), an owner **submits** a proposal with
   `ProposalAction::Call(CallData { target: vesting_id, function: ..., args: ... })`.
4. Other owners **confirm** until the threshold is met.
5. Anyone calls `execute()`.  The multisig contract address invokes the vesting
   function, satisfying `admin.require_auth()` inside the vesting contract.

An end-to-end test for this flow is in `test_multisig_create_schedule` inside
`contracts/vesting/src/lib.rs`.

---

## 7. What is the difference between Completed and Revoked status?

Both statuses indicate a schedule is no longer active.  The difference is how
they are reached:

| Status | How reached | Tokens |
|--------|-------------|--------|
| `Completed` | Beneficiary claimed the full `total_amount` | All tokens transferred to beneficiary |
| `Revoked` | Admin called `revoke()` before full vesting | Vested-but-unclaimed tokens go to beneficiary; unvested tokens go to the `recipient` address passed to `revoke()` |

In both cases `get_claimable()` returns `0` because:

- **Completed:** `claimed_amount == total_amount`, so `vested - claimed = 0`.
- **Revoked:** `revoke()` sets `claimed_amount = vested_amount` before
  transferring tokens, so `vested - claimed = 0` for the current timestamp.

Neither a Completed nor a Revoked schedule can be claimed or revoked again —
both operations check `schedule.status == Active` and panic with
`VestingError::ScheduleNotActive` otherwise.

Source: `ScheduleStatus` enum and `claim()` / `revoke()` in
`contracts/vesting/src/lib.rs`.

---

## 8. What does extend\_ttl() do and when should I call it?

Soroban smart contract storage entries expire after a configurable number of
ledgers (the TTL — time-to-live).  If a contract instance or a persistent
entry is not accessed within its TTL it is **archived**: it still exists on
the ledger but cannot be read until it is explicitly restored.

State-changing entry points (`create_schedule`, `claim`, `revoke`, `confirm`,
`execute`, etc.) automatically call `bump_instance()` and extend the TTL of
any persistent entries they touch, keeping them alive for up to
`INSTANCE_BUMP_LEDGERS = 3 110 400` ledgers (~180 days).

Read-only getters (`get_schedule`, `get_claimable`, `get_proposal`, etc.)
do **not** extend the TTL because they carry no write footprint.

`extend_ttl()` is a **permissionless** keep-alive:

```bash
stellar contract invoke --id "$VESTING_ID" -- extend_ttl
```

Call it from an off-chain keep-alive bot (cron job, monitoring script) when
the contract is in a long idle period — for example, a schedule that will not
be claimed for months.  A good heuristic is to call it when the remaining
TTL drops below `INSTANCE_BUMP_THRESHOLD = 518 400` ledgers (~30 days).

Source: `extend_ttl()` and TTL constant definitions in all three contract
source files; see also `docs/ttl.md` for the full TTL strategy.

---

## 9. How do I restore an archived contract entry?

If an entry's TTL reaches zero it is archived.  The entry still exists on the
ledger as a ledger entry with an `Archived` state but cannot be read by the
contract until restored.

To restore it, submit a Soroban restore transaction using `stellar-cli`:

```bash
# Restore the vesting contract instance
stellar contract restore \
  --id "$VESTING_ID" \
  --source my-key --network testnet

# Restore a specific persistent entry (e.g. Schedule #5)
# You will need the XDR key for the entry — derive it off-chain from
# DataKey::Schedule(5) using the Stellar SDK.
```

After restoration the entry has a fresh TTL equal to the network minimum.
Call `extend_ttl()` immediately afterwards to bump it back to the full
~180-day window.

For full details on what gets archived, how to calculate keys, and how to
tune TTL constants for production, see [`docs/ttl.md`](ttl.md).

---

## 10. Is this safe to use on mainnet?

**No.  Not recommended.**

The contracts are unaudited.  Known limitations:

- No professional security audit has been performed.
- The platform is a reference implementation for learning and prototyping.
- The backend API returns documented stubs for mutating operations unless
  contract IDs and a signing key are configured.
- The frontend has no Freighter / wallet integration.

If you intend to deploy with real value, you should:

1. Commission a professional Soroban smart-contract audit.
2. Review the trust model in [`docs/security-model.md`](security-model.md).
3. Review the disclosure policy in [`SECURITY.md`](../SECURITY.md).
4. Tune TTL constants for your expected usage patterns (see `docs/ttl.md`).

The Testnet deployment (contract IDs in `deployments.json` and the README) is
for demonstration only and carries no real value.

---

## 11. What does get\_claimable() return for Completed or Revoked schedules?

`get_claimable()` returns `0` for both statuses.

The implementation does not branch on `schedule.status`; it computes:

```rust
(vested_amount(&schedule, now) - schedule.claimed_amount).max(0)
```

- **Completed:** `claimed_amount == total_amount` and
  `vested_amount >= total_amount`, so the difference is `<= 0`, clamped to `0`.
- **Revoked:** `revoke()` sets `claimed_amount = vested_amount` before
  transferring tokens, so `vested_amount - claimed_amount == 0`.

Both paths are tested in `test_get_claimable_on_completed_schedule` and
`test_get_claimable_on_revoked_schedule` in `contracts/vesting/src/lib.rs`.

---

## 12. How do owner-set changes (add/remove/threshold) work in the multisig?

There are **no public entry points** for `add_owner`, `remove_owner`, or
`update_threshold`.  Every change to the owner set or threshold must go
through the full proposal lifecycle:

1. Submit a proposal with `ProposalAction::AddOwner(addr)`,
   `ProposalAction::RemoveOwner(addr)`, or `ProposalAction::UpdateThreshold(n)`.
2. Reach the threshold with owner confirmations.
3. Call `execute()`.

`execute()` handles these variants internally without a cross-contract call
(Soroban does not allow a contract to invoke itself).

**Confirmation invalidation on owner removal:** When an owner is removed,
any pending proposals they had previously confirmed are updated — their
`Confirm` key is deleted and `confirmation_count` is decremented by 1.  This
ensures that threshold accounting stays accurate after a removal.

Constraints enforced at execution time:

- `RemoveOwner` panics with `"cannot remove: would breach threshold"` if
  `owner_count <= threshold` after removal.
- `UpdateThreshold` panics with `"invalid threshold"` if the new value is `0`
  or greater than the current owner count.
- `AddOwner` panics with `"already an owner"` for a duplicate address.

Source: `internal_add_owner()`, `internal_remove_owner()`,
`internal_update_threshold()` in `contracts/multisig/src/lib.rs`.

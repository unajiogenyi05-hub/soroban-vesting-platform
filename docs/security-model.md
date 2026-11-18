# Security Model

This document provides a systematic per-function authorization model, trust
hierarchy, and threat analysis for the Soroban Vesting Platform. It is written
from the source code and existing documentation, and is intended to help
auditors, security researchers, and integrators reason about the contract
security without reading all the source code.

> **Unaudited.** This codebase has not been formally audited. Do not deploy
> with real value on mainnet without a professional security audit.

---

## 1. Trust Hierarchy

There are four distinct principals in this system, ordered from most to least
privileged:

| Principal | Description |
|-----------|-------------|
| **Admin** | A Stellar address (or the multisig contract address) that can create schedules, revoke, pause/unpause, and transfer its own role. Set once at deploy time via constructor — there is no separate `initialize` call, eliminating initialization front-running. |
| **Multisig owners** | A set of N addresses whose collective threshold-of-M confirmations are required before any multisig `ProposalAction` executes. When the multisig contract is the vesting admin, no single key can perform admin operations unilaterally. |
| **Beneficiary** | The recipient of a vesting schedule. Can only call `claim` on their own schedule. Has no admin rights. |
| **Anyone** | Permissionless callers. Can call read-only view functions and the `extend_ttl` keep-alive entry points. Can also trigger `execute()` on the multisig once the threshold is already met (the consent is expressed by owner confirmations, not by the executor). |

The backend API layer is trust-neutral: it constructs unsigned XDR for
client-side signing and **never holds private keys**. It cannot move tokens
unilaterally.

---

## 2. Per-Function Authorization Table

### 2.1 Vesting Contract (`contracts/vesting/src/lib.rs`)

| Function | Required Authorization | Pause-gated | Notes |
|----------|------------------------|:-----------:|-------|
| `__constructor(admin)` | — (deploy-time only) | — | Runs atomically with deployment; no separate init step |
| `create_schedule(params)` | `admin.require_auth()` | ✓ yes | Pulls tokens from `params.from` via `token::transfer` |
| `batch_create_schedule(params)` | `admin.require_auth()` | ✓ yes | Iterates `params`, delegates to `create_schedule_internal`; atomic all-or-nothing |
| `claim(schedule_id)` | `schedule.beneficiary.require_auth()` | ✓ yes | Only the beneficiary of that specific schedule can claim |
| `revoke(schedule_id, recipient)` | `admin.require_auth()` | ✗ no | Admin must recover funds even during a freeze; vested tokens auto-paid to beneficiary |
| `pause()` | `admin.require_auth()` | — | Sets paused flag; blocks `create_schedule`, `batch_create_schedule`, and `claim` |
| `unpause()` | `admin.require_auth()` | — | Clears paused flag |
| `transfer_admin(new_admin)` | `admin.require_auth()` + `new_admin.require_auth()` | ✗ no | Both parties must sign; new admin cannot be forced on an unwilling recipient |
| `extend_ttl()` | none (permissionless) | — | Keep-alive only; no state change |
| `get_schedule(id)` | none | — | Read-only |
| `get_claimable(id)` | none | — | Read-only |
| `get_beneficiary_schedules(beneficiary)` | none | — | Read-only |
| `get_admin()` | none | — | Read-only |
| `is_paused()` | none | — | Read-only |
| `schedule_count()` | none | — | Read-only |

### 2.2 Token Contract (`contracts/token/src/lib.rs`)

| Function | Required Authorization | Pause-gated | Notes |
|----------|------------------------|:-----------:|-------|
| `__constructor(admin, name, symbol, decimals, initial_supply)` | — | — | Mints `initial_supply` to admin at deploy time |
| `mint(to, amount)` | `admin.require_auth()` | ✓ yes | Creates new supply |
| `burn(from, amount)` | `from.require_auth()` | ✓ yes | Destroys caller's own tokens |
| `transfer(from, to, amount)` | `from.require_auth()` | ✓ yes | |
| `transfer_from(spender, from, to, amount)` | `spender.require_auth()` | ✓ yes | Requires prior `approve` from `from` |
| `approve(owner, spender, amount)` | `owner.require_auth()` | ✓ yes | Sets spending allowance |
| `pause()` | `admin.require_auth()` | — | |
| `unpause()` | `admin.require_auth()` | — | |
| `transfer_admin(new_admin)` | `admin.require_auth()` + `new_admin.require_auth()` | ✗ no | Same dual-auth pattern as vesting |
| `extend_ttl()` | none (permissionless) | — | Keep-alive only |
| `name()` / `symbol()` / `decimals()` | none | — | Read-only; no write footprint |
| `total_supply()` / `balance(account)` / `allowance(owner, spender)` | none | — | Read-only |
| `is_paused()` / `admin()` | none | — | Read-only |

### 2.3 Multisig Contract (`contracts/multisig/src/lib.rs`)

| Function | Required Authorization | Notes |
|----------|------------------------|-------|
| `__constructor(owners, threshold)` | — | Sets owner list and threshold atomically at deploy time |
| `submit(proposer, action, description)` | `proposer.require_auth()`, proposer must be an owner | Creates a pending proposal |
| `confirm(owner, proposal_id)` | `owner.require_auth()`, owner must be in owner list | Adds one confirmation |
| `revoke_confirmation(owner, proposal_id)` | `owner.require_auth()` | Removes own confirmation |
| `execute(proposal_id)` | none (permissionless once threshold met) | Dispatches `ProposalAction`; consent was expressed via confirmations |
| `cancel(caller, proposal_id)` | `caller.require_auth()`, caller must be original proposer | |
| `extend_ttl()` | none (permissionless) | Keep-alive only |
| `get_proposal(id)` / `get_owners()` / `get_threshold()` | none | Read-only |
| `proposal_count()` / `has_confirmed(id, owner)` / `is_owner(address)` | none | Read-only |

**Owner-set governance:** There are no public `add_owner`, `remove_owner`, or
`update_threshold` entry points. All changes to the owner set or threshold must
go through a fully-confirmed `ProposalAction::AddOwner`,
`ProposalAction::RemoveOwner`, or `ProposalAction::UpdateThreshold` proposal.
This means no single key can alter the governance structure unilaterally.

---

## 3. Pause Semantics

The vesting and token contracts each have a `pause` flag set by the admin.
The pause is not a blanket lock — functions that must remain available during
an incident are intentionally excluded.

### Vesting contract

| Function | Pause-gated? | Rationale |
|----------|:------------:|-----------|
| `create_schedule` | ✓ yes | No new funds should be locked during an incident |
| `batch_create_schedule` | ✓ yes | Same reason as `create_schedule` |
| `claim` | ✓ yes | Outbound transfers halted during an incident |
| `revoke` | ✗ no | Admin must recover unvested funds and pay out already-vested tokens even while paused; a paused contract must never become a permanent fund lock |
| `transfer_admin` | ✗ no | Admin hand-off must always be possible; blocking it could leave the contract permanently stuck with no way to unpause |

### Token contract

| Function | Pause-gated? | Rationale |
|----------|:------------:|-----------|
| `mint` | ✓ yes | No new supply during an incident |
| `burn` | ✓ yes | Consistent with `mint` |
| `transfer` / `transfer_from` | ✓ yes | All token movement halted |
| `approve` | ✓ yes | No new spending allowances while paused |
| `transfer_admin` | ✗ no | Same reason as vesting |

The multisig contract has no pause mechanism. It acts as the vesting admin and
can call `pause()` / `unpause()` on the vesting contract via normal proposal
flow.

---

## 4. Checks-Effects-Interactions (CEI) Compliance

The Soroban execution model is single-threaded within a transaction, which
eliminates classical re-entrancy. However, the contracts still follow the CEI
pattern to produce safe and auditable code.

### Vesting `claim()`

1. **Checks** — Schedule exists, is Active, claimable > 0.
2. **Effects** — Increment `claimed_amount`; mark Completed if fully vested; write updated schedule to persistent storage.
3. **Interactions** — Transfer tokens to beneficiary via `token::Client::transfer`.

### Vesting `revoke()`

1. **Checks** — Schedule exists, is Active.
2. **Effects** — Record `unclaimed_vested` in `claimed_amount`, set status to Revoked, write + extend TTL on the persistent schedule entry.
3. **Interactions** — Transfer `unclaimed_vested` to beneficiary, then transfer `unvested` to `recipient`.

State is fully committed before any external token transfers. An exception in
a token call would roll back the whole transaction (Soroban atomicity), not
leave the schedule in a partially-mutated state.

### Multisig `execute()`

1. **Checks** — Proposal exists, is Pending, confirmation count ≥ threshold.
2. **Effects** — Mark proposal Executed in persistent storage.
3. **Interactions** — Dispatch `env.invoke_contract` for `ProposalAction::Call`; handle owner-management actions internally.

---

## 5. Known Limitations

| Limitation | Detail |
|------------|--------|
| **No upgrade mechanism** | Contracts are immutable once deployed. A bug requires deploying a new contract and migrating state manually. |
| **Integer division rounding** | `vested_amount` divides `elapsed × total_amount / total_duration`. The result is truncated (floor division). A beneficiary may receive 1 stroop less than the theoretical continuous-time amount when `elapsed` is not an exact multiple of `total_duration`. The surplus is paid out at full-vest or remains in the contract until the schedule is revoked. |
| **No proptest in WASM target** | Property-based tests run against the native target only. The Soroban WASM runtime is not tested by proptest. |
| **No audit** | Unaudited. Do not deploy with real value without a professional security audit. |
| **No mainnet deployment** | Testnet only. Contract IDs in `deployments.json` have no real value. |
| **Beneficiary list not bounded** | `get_beneficiary_schedules` stores a `Vec<u64>` per address in persistent storage. There is no cap on how many schedules a single beneficiary can accumulate, which could grow the storage entry unboundedly over time. |

---

## 6. Attack Surface Summary

This section maps the attack surface to the categories in [SECURITY.md](../SECURITY.md).

### Smart Contracts (Critical)

| Vector | Mitigation |
|--------|------------|
| Authorization bypass on `create_schedule`, `revoke`, `pause`, `transfer_admin` | `admin.require_auth()` enforced on every call; fails at the Soroban host level if auth is absent |
| Authorization bypass on `claim` | `schedule.beneficiary.require_auth()` enforced; only the exact beneficiary address can claim their schedule |
| Unilateral admin takeover when multisig is admin | No public `add_owner`/`remove_owner`/`update_threshold` entry points; all owner-set changes require a threshold-confirmed proposal |
| Integer overflow in `vested_amount` | `saturating_mul` used for `elapsed × total_amount`; `checked_add` used to reject schedules with overflowing `start_time + cliff_duration` or `start_time + total_duration` |
| Mint overflow creating excess supply | `checked_add` on both per-account balance and total supply in `token::_mint` |
| Initialization front-running | Eliminated by constructor-based deployment; args supplied atomically at deploy time |
| Permanent fund lock via pause | `revoke` is intentionally not pause-gated so the admin can always recover funds |
| Permanent fund lock via TTL expiry | `bump_instance` on every public call + `extend_ttl` permissionless keep-alive; persistent entries bumped on every write; bump target is the network `max_entry_ttl` (3 110 400 ledgers ≈ 180 days) |
| Partial batch leaving state inconsistent | `batch_create_schedule` runs inside a single transaction; Soroban atomicity means any validation failure rolls back all state changes |
| Re-entrancy via token callbacks | Soroban is single-threaded within a transaction; additionally, CEI ordering ensures state is committed before token transfers |
| `DataKey` storage collision | Enum variants `Schedule(u64)` and `BeneficiarySchedules(Address)` are distinct XDR-encoded keys; no collision is possible |

### Backend (High)

| Vector | Mitigation |
|--------|------------|
| Private key logging | Backend never holds private keys; constructs unsigned XDR for client-side signing only |
| Input injection via Stellar addresses or amounts | Inputs passed to Soroban SDK types (`Address`, `nativeToScVal`) which enforce valid encoding |
| Rate limit bypass | `express-rate-limit` middleware applied globally before route handlers |
| Inconsistent error shape leaking stack traces | Centralised error handler always returns `{ error: string, code: string }`; raw stack is logged server-side only |

### Frontend (Medium)

| Vector | Mitigation |
|--------|------------|
| XSS via API response data | DOM updates use `textContent` not `innerHTML` |
| Sensitive data in localStorage | No private keys or seed phrases are stored client-side |

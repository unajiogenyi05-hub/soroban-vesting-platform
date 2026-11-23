# Storage TTL Reference

Soroban persistent and instance storage entries expire after a certain number
of ledgers.  Expired **persistent** entries are archived by the network and can
be restored (they are not deleted).  Expired **instance** storage, however,
makes the entire contract unusable until the instance is restored.

Because instance storage holds the core state of every contract in this
platform (owners, admin, paused flag, counters), a contract with an expired
instance cannot be called.  Every public entry point therefore bumps the
instance TTL on each call.

## Constants (all three contracts)

| Constant | Value | Approx. | Used for |
|----------|-------|---------|---------|
| `INSTANCE_BUMP_LEDGERS` | 3 110 400 | ~180 days | Extend instance TTL to |
| `INSTANCE_BUMP_THRESHOLD` | 518 400 | ~30 days | Only extend when TTL < this |
| `PERSISTENT_BUMP_LEDGERS` | 3 110 400 | ~180 days | Extend persistent entry TTL to |
| `PERSISTENT_BUMP_THRESHOLD` | 518 400 | ~30 days | Only extend when TTL < this |

Ledger cadence assumption: ~5 seconds per ledger on Stellar mainnet/testnet.

### Why 3 110 400 and not a larger value

The Soroban protocol enforces a hard ceiling called `max_entry_ttl`.  Any
`extend_ttl` call requesting more ledgers than `max_entry_ttl` is silently
clamped to `max_entry_ttl` by the host — the transaction does not fail, but the
resulting TTL is lower than requested.  Using a value that exceeds
`max_entry_ttl` is therefore misleading in documentation and wastes rent fees.

`max_entry_ttl = 3 110 400` ledgers (≈ 180 days at 5 s/ledger) as set in
`stellar-core/soroban-settings/testnet_settings_upgrade.json` (protocol 29,
applies to both testnet and mainnet).  The constants in this repository use
exactly that ceiling.

Source: <https://github.com/stellar/stellar-core/blob/master/soroban-settings/testnet_settings_upgrade.json>


## What expires and how to restore it

### Instance storage

**Contract:** multisig, vesting, token  
**Keys:** OWNERS, THRESHOLD, PROP\_COUNT (multisig); ADMIN, PAUSED, SCHED\_ID (vesting);
ADMIN, PAUSED, TOTAL, NAME, SYMBOL, DECIMALS (token)

If instance storage is archived, the contract cannot be invoked at all until
it is restored.  Use `stellar contract restore` or the Stellar SDK's
`restoreFootprint` operation targeting the contract instance.

A helper script is provided at `scripts/restore.sh` which restores the instance
storage for the token and vesting contracts using the IDs in `deployments.json`:

```bash
./scripts/restore.sh
```

Persistent entries (vesting schedules, token balances/allowances, multisig
proposals/confirmations) must be restored individually by storage key — see the
example below and the script comments for details.

Instance TTL is bumped on **every public function call**, so an actively used
contract will never archive.

### Persistent storage

**Contract:** vesting  
**Keys:** `Schedule(id)`, `BeneficiarySchedules(address)`  
Bumped on every write (`create_schedule`, `claim`, `revoke`).

**Contract:** multisig  
**Keys:** `Proposal(id)`, `Confirm(proposal_id, address)`  
Bumped on every write (`submit`, `confirm`, `revoke_confirmation`, `execute`, `cancel`).

**Contract:** token  
**Keys:** `Balance(address)`, `Allowance(owner, spender)`  
Bumped on every write (`mint`, `burn`, `transfer`, `transfer_from`, `approve`).

If a persistent entry is archived it can be restored with:

```bash
stellar contract restore \
  --id <CONTRACT_ID> \
  --source <ACCOUNT> \
  --network testnet \
  --key <XDR_KEY>
```

For a step-by-step recovery procedure covering detection, verification, and
prevention see [docs/disaster-recovery.md](disaster-recovery.md).

## Choosing constants

The threshold/extend pair follows the standard Soroban pattern: only extend
when there are fewer than `THRESHOLD` ledgers remaining, and extend to
`BUMP_LEDGERS`.  This avoids paying rent on every call when the TTL is already
healthy.

For a production deployment adjust the constants to match the expected call
frequency and risk tolerance.  A contract that is called at least once a month
(> 518 400 ledgers at 5 s/ledger) will never approach expiry with the defaults
above.

## Storage cost estimation

Soroban charges a **resource fee** (rent) for every `extend_ttl` call. Operators
running `extend_ttl` as a keep-alive cron job should budget for these fees.

### How Stellar calculates rent for TTL extension

The rent fee for extending a ledger entry is proportional to:

1. **Entry size** — the byte size of the entry being extended (instance or
   persistent).
2. **Ledgers extended** — how many ledgers the TTL is pushed forward.
3. **Network write fee** — a dynamic network parameter (stroops per 1 KB), which
   grows as total bucket-list storage grows.

The approximate formula used by the network is:

```
rent_fee (stroops) = (size_bytes × ledgers × write_fee_per_1kb)
                   / (1024 × rate_denominator)
```

| Storage type | Rate denominator | Notes |
|-------------|-----------------|-------|
| Persistent  | 4,096           | Used for `Schedule(id)` and `BeneficiarySchedules(addr)` entries |
| Instance    | 4,096           | Used for the contract instance entry (`ADMIN`, `PAUSED`, `SCHED_ID`, …) |
| Temporary   | 2,048           | Not used by this platform |

`1 XLM = 10,000,000 stroops`.

Because `write_fee_per_1kb` is dynamic and entry sizes vary by contract, treat
any figure below as an **estimate**. Always confirm with a live simulation
before committing to a budget.

### Worked example: 3 contract instances × 3,110,400 ledgers

`3,110,400 ledgers ≈ 180 days` at ~5 seconds per ledger — this is exactly
`INSTANCE_BUMP_LEDGERS`, the amount each `extend_ttl(env)` call adds to a
contract instance.

Assume each contract instance entry is ~200 bytes (~0.2 KB) and use the
mainnet reference point from the [Stellar rent calculator]
(https://leighmcculloch.github.io/soroban-rent-calculator/): 1 KB extended by
17,280 ledgers (30 days) at rate denominator 4,096 costs ≈ 1,267 stroops.

Scaling to 3,110,400 ledgers (180 days = 6 × 30 days):

- Per KB for 3,110,400 ledgers: `1,267 × 6 ≈ 7,602 stroops` (~0.00076 XLM)
- Per 200-byte instance: `7,602 × 0.2 ≈ 1,520 stroops` (~0.000152 XLM)
- **3 instances: `1,520 × 3 ≈ 4,560 stroops` (~0.000456 XLM)**

So extending 3 contract instances by 3,110,400 ledgers costs on the order of
**0.0005 XLM** under these assumptions — a small fraction of a cent. Costs
scale linearly: doubling the entry size or the ledger count doubles the fee.
On testnet the write fee is typically lower, often near-zero.

### Getting an exact figure

Before scheduling recurring `extend_ttl` calls, measure the real cost on your
target network:

1. **Stellar rent calculator** — quick estimate for a given entry size and TTL:
   https://leighmcculloch.github.io/soroban-rent-calculator/
2. **RPC simulation** — submit a `simulateTransaction` for the `extend_ttl`
   call and read the returned `resourceFee` (plus the base inclusion fee). This
   is the most accurate method because it reflects the current dynamic write
   fee and your actual entry sizes:
   https://developers.stellar.org/docs/smart-contracts/resource-limits-fees

```bash
# Example: simulate an extend_ttl call to inspect the resource fee before
# sending it for real.
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <ANY_ACCOUNT> \
  --network testnet \
  -- extend_ttl \
  --simulate   # (or use the RPC `simulateTransaction` method directly)
```

> **Tip:** Budget with headroom. If `write_fee_per_1kb` rises as the ledger
> grows, future `extend_ttl` calls will cost more than today's estimate. Re-run
> the simulation periodically and adjust the cron budget accordingly.

## Read-only getters and extend_ttl()

Pure getters (`get_*`, `is_*`, `name`, `symbol`, `decimals`, `total_supply`,
`balance`, `allowance`, `admin`, `proposal_count`, `has_confirmed`,
`is_owner`, `schedule_count`) do **not** call `bump_instance()`.  This keeps
them as genuinely read-only calls — they carry no write footprint, so the CLI
submits them as simulations rather than transactions, and callers pay no rent
fee.

Each contract exposes a permissionless `extend_ttl(env)` function that bumps
the instance TTL to `INSTANCE_BUMP_LEDGERS`.  An off-chain keep-alive bot
(or any holder of XLM) can call this once every ~150 days to prevent the
instance from being archived.

```bash
# Example: keep-alive call via stellar CLI
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <ANY_ACCOUNT> \
  --network mainnet \
  -- extend_ttl
```

For shell and Node.js examples and a full scheduling guide (cron and GitHub
Actions) see [docs/offchain-keepalive.md](offchain-keepalive.md).

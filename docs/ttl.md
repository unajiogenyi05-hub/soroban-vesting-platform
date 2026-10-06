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

## Choosing constants

The threshold/extend pair follows the standard Soroban pattern: only extend
when there are fewer than `THRESHOLD` ledgers remaining, and extend to
`BUMP_LEDGERS`.  This avoids paying rent on every call when the TTL is already
healthy.

For a production deployment adjust the constants to match the expected call
frequency and risk tolerance.  A contract that is called at least once a month
(> 518 400 ledgers at 5 s/ledger) will never approach expiry with the defaults
above.

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

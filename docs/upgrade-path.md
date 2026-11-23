# Upgrade path

Soroban contracts are immutable once deployed. This guide covers the full
migration process when you need to ship new logic: why no in-place upgrade
exists, how to move state to the new contract, how to hand over admin
authority, how to decommission the old contract safely, and how to validate
everything on Testnet before touching Mainnet.

---

## 1. Why Soroban contracts are not upgradeable in-place

The WASM bytecode stored at a contract address is part of the ledger entry and
cannot be replaced after deployment. There is no `upgrade_wasm` or
`set_code` instruction in the Soroban host. The contract address is derived
from the deployer's account and a salt at deploy time, and the bytecode hash is
fixed from that point on.

What this means in practice:

- Bugs in contract logic cannot be patched in situ.
- New entry points or changed argument types require a new deployment.
- The old contract address continues to exist on-chain indefinitely (ledger
  entries do not vanish; they can only be archived when their TTL expires, and
  even then the data is still recoverable via a restore).

The only path to new logic is:

1. Deploy a new contract with the corrected/extended WASM.
2. Migrate the relevant state from the old contract to the new one.
3. Point admin authority at the new contract.
4. Stop using the old contract address.

---

## 2. State migration strategy (export → import)

### 2.1 Export state off-chain

All the data you need is readable via public, read-only getters. No transaction
fees are incurred for simulation calls. Run the following using `stellar
contract invoke` or the backend's Soroban RPC helper.

**Enumerate every schedule:**

```bash
# Total number of schedules ever created (IDs run from 1 to schedule_count)
TOTAL=$(stellar contract invoke --id "$OLD_VESTING_ID" \
  --source admin --network testnet \
  -- schedule_count)

# Fetch each schedule in turn
for ID in $(seq 1 "$TOTAL"); do
  stellar contract invoke --id "$OLD_VESTING_ID" \
    --source admin --network testnet \
    -- get_schedule --schedule_id "$ID"
done
```

Each `get_schedule` call returns a `VestingSchedule` struct with these fields:

| Field | Type | Notes |
|-------|------|-------|
| `id` | `u64` | Sequential, starts at 1 |
| `beneficiary` | `Address` | Recipient |
| `token` | `Address` | Token contract address |
| `total_amount` | `i128` | Original locked amount |
| `claimed_amount` | `i128` | Already paid out — **must be recorded** |
| `start_time` | `u64` | Unix timestamp (seconds) |
| `cliff_duration` | `u64` | Seconds before anything vests |
| `total_duration` | `u64` | Total seconds of linear vesting |
| `status` | enum | `Active`, `Revoked`, or `Completed` |

Export only `Active` schedules. `Revoked` and `Completed` schedules have no
remaining obligation and do not need to be replayed on the new contract.

**Export token balances:**

```bash
# Balance held by the old vesting contract
stellar contract invoke --id "$OLD_TOKEN_ID" \
  --source admin --network testnet \
  -- balance --id "$OLD_VESTING_ID"
```

If you are deploying a new token contract alongside the new vesting contract,
capture every holder's balance via the same `balance` call.

**Export beneficiary index (optional cross-check):**

```bash
stellar contract invoke --id "$OLD_VESTING_ID" \
  --source admin --network testnet \
  -- get_beneficiary_schedules --beneficiary "$BENEFICIARY_ADDRESS"
```

Use this to verify that every schedule ID returned by `get_beneficiary_schedules`
matches the full scan from `schedule_count`.

### 2.2 Adjust for already-claimed amounts

`claimed_amount` records how many tokens have already left the old contract for
a given beneficiary. The new contract cannot know about past claims unless you
encode them. Two approaches:

**Option A — reduce total_amount (simpler):**

Create the schedule on the new contract with:

```
new_total_amount = old_total_amount - old_claimed_amount
```

The beneficiary starts fresh: nothing is pre-claimed, but the schedule only
covers the remaining unvested balance. `start_time`, `cliff_duration`, and
`total_duration` stay the same. If the schedule has already passed the cliff, the
beneficiary will be able to claim immediately after `start_time` based on the
time elapsed.

> **Limitation:** the beneficiary's historical claim record is lost. If that
> matters for accounting, use Option B instead.

**Option B — preserve claimed_amount offset (accurate):**

Create the schedule with the original `total_amount`, then record the
`claimed_amount` offset explicitly in a separate off-chain ledger or inside the
new contract if you add a migration-specific entry point. The new contract's
`get_claimable` computation is `vested_amount(now) - claimed_amount`, so
seeding `claimed_amount` correctly produces exact continuity.

### 2.3 Replay schedules on the new contract

Use `batch_create_schedule` to replay groups of schedules in a single
transaction. This costs fewer fees than individual `create_schedule` calls and
is atomic — partial state is impossible if the transaction fails.

```bash
# Example: batch-import two Active schedules (Option A: reduced total_amount)
stellar contract invoke --id "$NEW_VESTING_ID" \
  --source admin --network testnet \
  -- batch_create_schedule \
  --params '[
    {
      "from": "'"$ADMIN"'",
      "beneficiary": "'"$BEN1"'",
      "token_address": "'"$NEW_TOKEN_ID"'",
      "total_amount": 750000,
      "start_time": 1700000000,
      "cliff_duration": 7776000,
      "total_duration": 31536000
    },
    {
      "from": "'"$ADMIN"'",
      "beneficiary": "'"$BEN2"'",
      "token_address": "'"$NEW_TOKEN_ID"'",
      "total_amount": 490000,
      "start_time": 1700000000,
      "cliff_duration": 0,
      "total_duration": 15768000
    }
  ]'
```

`batch_create_schedule` requires admin auth and the contract must not be paused.
It returns a `Vec<u64>` of the newly created schedule IDs. Cross-check those
IDs against your export to confirm every schedule was created.

### 2.4 Migrate token balances

If the new deployment uses a new token contract, mint balances to match the old
state:

```bash
# Mint the total locked amount into the new vesting contract
stellar contract invoke --id "$NEW_TOKEN_ID" \
  --source admin --network testnet \
  -- mint \
  --to "$NEW_VESTING_ID" \
  --amount "$TOTAL_UNVESTED_LOCKED"
```

The `mint` call requires admin auth on the token contract. The amount to mint
equals the sum of `(total_amount - claimed_amount)` for every Active schedule
you imported — exactly what the new vesting contract will need to pay out.

If you are reusing the same token contract (not deploying a new one), you
instead need a `transfer` from the old vesting contract's balance to the new
one. Because the old vesting contract holds those tokens in its own address,
the only way to move them without having the contract's private key is to call
`revoke` on each Active schedule with the new vesting contract address as the
`recipient`, then fund the new contract from there.

---

## 3. Admin pointer migration

The multisig currently holds admin authority over both the vesting and token
contracts. After the new contracts are deployed and state has been imported,
redirect that authority by calling `transfer_admin` on each contract.

`transfer_admin` requires auth from both the **current** admin (the multisig)
and the **new** admin. Submit two proposals — one per contract — through the
existing multisig flow.

### Step-by-step for the vesting contract

**Step 1 — Submit a proposal:**

```bash
# Proposer (an owner) submits a Call proposal targeting the old vesting contract
stellar contract invoke --id "$MULTISIG_ID" \
  --source owner1 --network testnet \
  -- submit \
  --proposer "$OWNER1_ADDRESS" \
  --action '{
    "Call": {
      "target": "'"$OLD_VESTING_ID"'",
      "function": "transfer_admin",
      "args": ["'"$NEW_VESTING_ID"'"]
    }
  }' \
  --description '"Transfer vesting admin to new contract v2"'
```

**Step 2 — Owners confirm to threshold:**

```bash
# Each required owner confirms the proposal (replace PROPOSAL_ID with the
# u64 returned by submit)
stellar contract invoke --id "$MULTISIG_ID" \
  --source owner2 --network testnet \
  -- confirm \
  --owner "$OWNER2_ADDRESS" \
  --proposal_id "$PROPOSAL_ID"
```

Repeat for as many owners as needed to reach the threshold. Check progress
with:

```bash
stellar contract invoke --id "$MULTISIG_ID" \
  --source admin --network testnet \
  -- get_proposal --proposal_id "$PROPOSAL_ID"
```

**Step 3 — Execute:**

```bash
# Once confirmation_count >= threshold, anyone can execute
stellar contract invoke --id "$MULTISIG_ID" \
  --source admin --network testnet \
  -- execute \
  --proposal_id "$PROPOSAL_ID"
```

`execute()` is permissionless once the threshold is met. The multisig calls
`transfer_admin(new_vesting_address)` on the old vesting contract. Because
the multisig is the current admin, `admin.require_auth()` inside the vesting
contract is satisfied by the cross-contract call origin.

The new vesting contract's `__constructor` sets it as its own admin at deploy
time. If you want the multisig to govern the new contract as well, run a
second `transfer_admin` call on the **new** vesting contract to hand admin
from the deployer key to the multisig address.

### Repeat for the token contract

Follow the same three steps, substituting `$OLD_TOKEN_ID` for the target and
`transfer_admin` with the new token admin address. If the token contract is
unchanged (you are reusing it), skip this step.

---

## 4. Decommissioning the old contract

Do not let the old contract silently fade away. Archival hides the state but
does not delete it; anyone who knows the old address can still try to interact
with it (or restore it). An explicit decommission prevents accidents.

### Step 1 — Revoke all remaining Active schedules

Call `revoke(schedule_id, treasury_address)` for every Active schedule on the
old contract. This pays out any vested-but-unclaimed tokens to the beneficiary
and returns unvested tokens to the treasury, draining the old contract's token
balance to zero.

```bash
for ID in $(jq '.active_schedule_ids[]' migration-export.json); do
  stellar contract invoke --id "$OLD_VESTING_ID" \
    --source admin --network testnet \
    -- revoke \
    --schedule_id "$ID" \
    --recipient "$TREASURY_ADDRESS"
done
```

> **Note:** `revoke` is not gated by the pause flag. You can run it even
> after pausing the contract (see Step 2).

### Step 2 — Pause the old contract via the multisig

Submit a `ProposalAction::Call` targeting `pause()` on the old vesting contract
and execute it through the multisig. This prevents any new `create_schedule` or
`claim` calls from succeeding even if someone accidentally uses the old address.

```bash
# Submit pause proposal
stellar contract invoke --id "$MULTISIG_ID" \
  --source owner1 --network testnet \
  -- submit \
  --proposer "$OWNER1_ADDRESS" \
  --action '{
    "Call": {
      "target": "'"$OLD_VESTING_ID"'",
      "function": "pause",
      "args": []
    }
  }' \
  --description '"Decommission: pause old vesting contract"'

# Confirm and execute as in Section 3
```

### Step 3 — Record the old contract as decommissioned in deployments.json

Open `deployments.json` and move the old entry to a `decommissioned` block:

```json
{
  "testnet": {
    "token": "C<NEW_TOKEN_ID>",
    "vesting": "C<NEW_VESTING_ID>",
    "multisig": "C<MULTISIG_ID>"
  },
  "decommissioned": {
    "testnet": {
      "vesting_v1": {
        "contract_id": "C<OLD_VESTING_ID>",
        "decommissioned_at": "2026-11-21",
        "reason": "Logic upgrade to v2 — issue #292"
      }
    }
  }
}
```

Do not delete the old ID from the file. Keeping it means anyone auditing the
repository can trace every contract address ever used.

### Step 4 — Do not send further funds to the old address

Add the old contract ID to any internal allowlists or treasury tooling as a
blocked address. The old contract will continue to exist on-chain, but with a
zero token balance and the pause flag set, it is inert.

---

## 5. Testing the migration on Testnet first

Never perform a state migration directly on Mainnet. Testnet uses identical
protocol rules, real network latency, and the actual Soroban host, so it
catches integration bugs that the local test harness cannot.

### Step 1 — Deploy new contracts to Testnet

```bash
# Build fresh WASM
stellar contract build

# Deploy new token (if changing the token contract)
export NEW_TOKEN_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/token.wasm \
  --source my-admin --network testnet \
  -- --admin "$ADMIN" --name '"VEST"' --symbol '"VST"' --decimals 7 --initial_supply 0)

# Deploy new vesting
export NEW_VESTING_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/vesting.wasm \
  --source my-admin --network testnet \
  -- --admin "$ADMIN")

echo "New vesting: $NEW_VESTING_ID"
echo "New token:   $NEW_TOKEN_ID"
```

### Step 2 — Export state from the old Testnet contract

```bash
# Export all schedules to a JSON file
node scripts/create-schedule-example.js \
  --export --network testnet \
  --vesting-id "$OLD_VESTING_ID" \
  > migration-export.json

# Verify export looks correct
cat migration-export.json | jq '.schedules | length'
```

Alternatively, iterate using the shell loop from Section 2.1 and collect
output into `migration-export.json` manually.

### Step 3 — Import state into the new Testnet contract

Replay `batch_create_schedule` calls as described in Section 2.3. After
importing, verify each schedule with:

```bash
for ID in $(jq '.schedules[].old_id' migration-export.json); do
  stellar contract invoke --id "$NEW_VESTING_ID" \
    --source admin --network testnet \
    -- get_schedule --schedule_id "$ID"
done
```

Check that `total_amount`, `start_time`, `cliff_duration`, and
`total_duration` match the export, and that `claimed_amount` reflects
the correct offset (zero for Option A, or the original value for Option B).

### Step 4 — Verify claimable amounts match

For each beneficiary, compare the claimable amount on the old contract against
the new contract at the same simulated timestamp:

```bash
# Old contract
stellar contract invoke --id "$OLD_VESTING_ID" \
  --source admin --network testnet \
  -- get_claimable --schedule_id 1

# New contract (should equal old result when using Option B,
# or old result minus already-claimed when using Option A)
stellar contract invoke --id "$NEW_VESTING_ID" \
  --source admin --network testnet \
  -- get_claimable --schedule_id 1
```

Spot-check at least three schedules: one before its cliff, one mid-vest, and
one near completion.

### Step 5 — Run the demo script against the new deployment

The demo script exercises the full schedule lifecycle end-to-end. Point it at
the new contract IDs:

```bash
VESTING_CONTRACT_ID="$NEW_VESTING_ID" \
TOKEN_CONTRACT_ID="$NEW_TOKEN_ID" \
  ./scripts/demo-testnet.sh
```

The script creates a schedule, advances the simulated ledger, claims tokens,
and verifies the beneficiary balance. A clean run is a prerequisite for
proceeding to Mainnet.

### Step 6 — Proceed on Mainnet only after Testnet passes

Once all five steps above complete without errors:

1. Repeat the deployment and import steps against `--network mainnet`.
2. Use `get_claimable_at` with the current Mainnet ledger timestamp to do
   a final sanity check on a sample of schedules before executing the admin
   pointer migration (Section 3).
3. Execute the multisig proposals to redirect admin authority.
4. Decommission the old Mainnet contract (Section 4).
5. Update `deployments.json` with the new Mainnet contract IDs.

---

## Quick reference: functions used in this guide

| Function | Contract | Purpose |
|----------|----------|---------|
| `schedule_count()` | Vesting | Total schedules ever created |
| `get_schedule(id)` | Vesting | Fetch full schedule struct (read-only) |
| `get_beneficiary_schedules(addr)` | Vesting | Schedule IDs for one beneficiary (read-only) |
| `get_claimable(id)` | Vesting | Claimable amount right now (read-only) |
| `get_claimable_at(id, ts)` | Vesting | Claimable at an arbitrary timestamp (read-only) |
| `batch_create_schedule(params)` | Vesting | Atomically create multiple schedules |
| `revoke(id, recipient)` | Vesting | Drain unvested tokens before decommission |
| `pause()` | Vesting | Block create/claim on decommissioned contract |
| `transfer_admin(new_admin)` | Vesting / Token | Hand over admin authority |
| `balance(account)` | Token | Read token balance (read-only) |
| `mint(to, amount)` | Token | Seed token balance on new contract |
| `submit(proposer, action, desc)` | Multisig | Submit a governance proposal |
| `confirm(owner, id)` | Multisig | Add an owner confirmation |
| `execute(id)` | Multisig | Execute once threshold is met |

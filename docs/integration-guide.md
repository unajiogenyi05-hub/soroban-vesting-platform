# Integrating the Vesting Platform with a DAO or Treasury

This guide covers integration design decisions after deployment. For building
and deploying the contracts, see [testnet-demo.md](testnet-demo.md). For the
contract roles and interaction model, see [architecture.md](architecture.md).
The platform is unaudited and should not hold real value without a professional
security review.

The examples use Testnet and shell variables such as `VESTING_ID`,
`TOKEN_ID`, `MULTISIG_ID`, `TREASURY`, and `OWNER_1`. Replace every
placeholder with a real contract or Stellar account address.

## 1. Deploying with multisig as vesting admin

Deploy the multisig first, then deploy or transfer the vesting admin role to
its contract address.

```bash
export NETWORK=testnet
export OWNER_1=$(stellar keys address dao-owner-1)

export MULTISIG_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/multisig.wasm \
  --source dao-owner-1 \
  --network "$NETWORK" \
  -- \
  --owners "[\"$OWNER_1\"]" \
  --threshold 1)

export VESTING_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/vesting.wasm \
  --source dao-owner-1 \
  --network "$NETWORK" \
  -- \
  --admin "$MULTISIG_ID")
```

For a production DAO, do not use a 1-of-1 multisig. Its contract address is
the vesting admin, and the multisig's owners are the only accounts that can
govern schedule creation, revocation, pause operations, and admin migration.

To transfer an existing vesting contract, invoke `transfer_admin` from the
current admin and authorize the new multisig as well:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-owner-1 \
  --network testnet \
  -- transfer_admin \
  --new_admin "$MULTISIG_ID"

stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-owner-1 \
  --network testnet \
  -- get_admin
```

The final output should be `"$MULTISIG_ID"`. The treasury account that funds
the schedules is separate from the vesting admin.

## 2. Choosing an N-of-M threshold

Deploy a multisig with `N` distinct owners and a threshold of `M`:

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/multisig.wasm \
  --source dao-owner-1 \
  --network testnet \
  -- \
  --owners "[\"$OWNER_1\",\"$OWNER_2\",\"$OWNER_3\",\"$OWNER_4\",\"$OWNER_5\"]" \
  --threshold 3
```

This is a 3-of-5 multisig. Use these as starting points, not as a substitute
for a governance risk assessment:

| Setup | Suitable for | Tradeoff |
|-------|--------------|----------|
| 1-of-M | Testnet-only demos | Fast, but any one owner controls vesting |
| 2-of-3 | Small protocol teams | Low coordination cost, one signer cannot act alone |
| 3-of-5 | DAO or treasury operations | Better liveness and key-loss tolerance; 40% of keys can stall the DAO |
| 4-of-7 | High-value treasury | Strong compromise resistance; more proposal coordination |

Threshold rules are enforced by the contract: it must be at least 1 and cannot
exceed the owner count. Remove-owner and threshold-change operations go through
confirmed proposals too; there are no direct owner-management entry points.
Choose isolated keys for different governance roles, back up recovery and
rotation procedures, and test owner removal and threshold changes on Testnet.

## 3. Creating schedules via multisig proposal

The treasury must hold the tokens and must authorize the vesting contract to
pull `total_amount`. Mint or transfer the tokens to the treasury first, then
approve the vesting contract as the treasury:

```bash
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source dao-treasury \
  --network testnet \
  -- approve \
  --owner "$TREASURY" \
  --spender "$VESTING_ID" \
  --amount 500000
```

The vesting contract does not use the allowance when this implementation calls
`token.transfer(from, ...)`; `from` must still hold enough tokens and must
authorize the transfer. Keeping the treasury as `from` is therefore normal,
while the multisig remains only the vesting admin.

Submit the schedule as a `ProposalAction::Call`:

```bash
START=$(date +%s)
BENEFICIARY="G..."

stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source dao-owner-1 \
  --network testnet \
  -- submit \
  --proposer "$OWNER_1" \
  --action "{ \"Call\": { \"target\": \"$VESTING_ID\", \"function\": \"create_schedule\", \"args\": [{ \"from\": \"$TREASURY\", \"beneficiary\": \"$BENEFICIARY\", \"token_address\": \"$TOKEN_ID\", \"total_amount\": \"500000\", \"start_time\": $START, \"cliff_duration\": 31536000, \"total_duration\": 126144000 }] } }" \
  --description '"Create DAO contributor vesting schedule"'
```

Save the returned proposal ID. Confirm it with enough owners for the threshold:

```bash
export PROPOSAL_ID=42

stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source dao-owner-1 \
  --network testnet \
  -- confirm \
  --owner "$OWNER_1" \
  --proposal_id "$PROPOSAL_ID"

stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source dao-owner-2 \
  --network testnet \
  -- confirm \
  --owner "$OWNER_2" \
  --proposal_id "$PROPOSAL_ID"
```

Once the threshold is met, anyone may execute it:

```bash
stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source dao-relayer \
  --network testnet \
  -- execute \
  --proposal_id "$PROPOSAL_ID"
```

The schedule ID is returned by `create_schedule`. Record it together with the
beneficiary and proposal ID in your operations database. Validate the schedule
before marking the proposal complete:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-relayer \
  --network testnet \
  -- get_schedule \
  --schedule_id 1
```

For multiple schedules, consider batching with a higher-level service only
after using Testnet to verify the exact JSON argument shape.

## 4. Monitoring claimable amounts

`get_claimable` is a read-only simulation and uses the ledger timestamp. It
does not write state or extend contract TTL:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-relayer \
  --network testnet \
  -- get_claimable \
  --schedule_id 1
```

A monitor should poll active schedule IDs, store the latest claimable amount,
and emit alerts for cliff expiry, claimable thresholds, or schedule status
changes. Polling with `get_claimable_at` can provide an off-chain projection
for a supplied Unix timestamp, but `get_claimable` is the current ledger value:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-relayer \
  --network testnet \
  -- get_claimable_at \
  --schedule_id 1 \
  --timestamp "$START"
```

Because reads do not refresh TTL, schedule instance keep-alives separately:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-relayer \
  --network testnet \
  --send=yes \
  -- extend_ttl

stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source dao-relayer \
  --network testnet \
  --send=yes \
  -- extend_ttl
```

Run these under a scheduled operations job, or call `extend_ttl` from another
contract mutating operation. `--send=yes` is required if the CLI identifies
the call as read-only. An archived instance or schedule must be restored before
it can be used; see [ttl.md](ttl.md).

## 5. Revoking a schedule as admin

Revocation is an admin operation, so submit it through the multisig. The
`recipient` is normally the DAO treasury. At revoke time, vested-but-unclaimed
tokens go to the beneficiary and unvested tokens return to `recipient`:

```bash
stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source dao-owner-1 \
  --network testnet \
  -- submit \
  --proposer "$OWNER_1" \
  --action "{ \"Call\": { \"target\": \"$VESTING_ID\", \"function\": \"revoke\", \"args\": [1, \"$TREASURY\"] } }" \
  --description '"Revoke schedule 1 and return unvested funds"'
```

Confirm and execute the returned proposal with the same threshold flow as
schedule creation:

```bash
export REVOKE_PROPOSAL_ID=43

stellar contract invoke --id "$MULTISIG_ID" --source dao-owner-1 \
  --network testnet -- confirm --owner "$OWNER_1" \
  --proposal_id "$REVOKE_PROPOSAL_ID"

stellar contract invoke --id "$MULTISIG_ID" --source dao-owner-2 \
  --network testnet -- confirm --owner "$OWNER_2" \
  --proposal_id "$REVOKE_PROPOSAL_ID"

stellar contract invoke --id "$MULTISIG_ID" --source dao-relayer \
  --network testnet -- execute --proposal_id "$REVOKE_PROPOSAL_ID"
```

Verify the status and treasury result:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-relayer \
  --network testnet \
  -- get_schedule \
  --schedule_id 1

stellar contract invoke \
  --id "$TOKEN_ID" \
  --source dao-relayer \
  --network testnet \
  -- balance \
  --account "$TREASURY"
```

Revocation marks the schedule inactive. Already-claimed tokens stay with the
beneficiary, and a revoked schedule cannot be reactivated. `revoke` is not
blocked when the vesting contract is paused.

For schedules that have not started, prefer a dedicated cancellation workflow
instead of revoke. Canceling returns the full amount to the recipient and
requires the schedule to be still before `start_time`.

## 6. Migrating the admin role

Admin migration changes who can create and revoke schedules. It does not
modify existing schedules or move token balances. First deploy and validate
the new multisig, then:

1. Verify the new multisig's owners and threshold.
2. Pause schedule creation if the migration is sensitive.
3. Submit `transfer_admin` against the vesting contract.
4. Confirm and execute it.
5. Verify `get_admin`.
6. Run one dry-run governance action before unpausing.

For a vesting contract whose current admin is a direct account, transfer it
from that account:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source current-admin \
  --network testnet \
  -- transfer_admin \
  --new_admin "$NEW_MULTISIG_ID"
```

`transfer_admin` requires authorization from both the current admin and the
new admin. `new_admin` must therefore be ready to authorize the handoff.

For a vesting contract already governed by `$OLD_MULTISIG_ID`, submit the
handoff through that multisig:

```bash
stellar contract invoke \
  --id "$OLD_MULTISIG_ID" \
  --source dao-owner-1 \
  --network testnet \
  -- submit \
  --proposer "$OWNER_1" \
  --action "{ \"Call\": { \"target\": \"$VESTING_ID\", \"function\": \"transfer_admin\", \"args\": [\"$NEW_MULTISIG_ID\"] } }" \
  --description '"Migrate vesting admin to new multisig"'
```

Confirm and execute that proposal using the old multisig's threshold. Anyone
may execute once the threshold is reached. Verify the result:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source dao-relayer \
  --network testnet \
  -- get_admin
```

Do not delete or decommission the old multisig until you have verified the
new admin and completed at least one successful Testnet-style governance
cycle. If contracts are being replaced rather than only the admin pointer,
follow [upgrade-path.md](upgrade-path.md) to export, recreate, and verify
schedules before retiring the old contract.

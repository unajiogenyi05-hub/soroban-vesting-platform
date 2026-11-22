# Testnet Demo — Full Vesting Lifecycle

This guide walks through deploying and using the Soroban Vesting Platform on
Stellar Testnet, from contract deployment to token claiming.

## Prerequisites

| Tool | Install |
|------|---------|
| Rust (stable) + wasm32v1-none target | `rustup target add wasm32v1-none` |
| stellar-cli | `curl -sSfL https://install.stellar.org \| sh` |
| jq | `apt install jq` / `brew install jq` |

## Automated demo

The fastest way to see the full flow:

```bash
git clone https://github.com/unajiogenyi05-hub/soroban-vesting-platform
cd soroban-vesting-platform
chmod +x scripts/demo-testnet.sh
./scripts/demo-testnet.sh
```

The script will:
1. Generate fresh admin and beneficiary keypairs
2. Fund both via Friendbot (testnet only)
3. Build all contracts from source
4. Deploy the token and vesting contracts (constructor args passed at deploy time)
5. Mint tokens, set allowance, create a 200-second vesting schedule with a
   60-second cliff
6. Claim vested tokens if the cliff has passed

## Manual walkthrough

### 1. Build contracts

```bash
stellar contract build
```

Outputs WASM files to `target/wasm32v1-none/release/`.

### 2. Fund an account

```bash
stellar keys generate my-admin --network testnet --fund
export ADMIN=$(stellar keys address my-admin)
```

### 3. Deploy the token contract

```bash
export TOKEN_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/token.wasm \
  --source my-admin \
  --network testnet \
  -- \
  --admin "$ADMIN" \
  --name '"My Token"' \
  --symbol '"MTK"' \
  --decimals 7 \
  --initial_supply 0)
echo "Token: $TOKEN_ID"
```

Constructor args are passed after `--` at deploy time. There is no separate
`initialize` call.

### 4. Deploy the vesting contract

```bash
export VESTING_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/vesting.wasm \
  --source my-admin \
  --network testnet \
  -- \
  --admin "$ADMIN")
echo "Vesting: $VESTING_ID"
```

The `--admin` arg is the constructor parameter. No separate `initialize` call is needed.

### 5. Mint and approve

```bash
# Mint tokens to admin
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source my-admin \
  --network testnet \
  -- mint \
  --to "$ADMIN" \
  --amount 1000000

# Approve vesting contract to pull tokens
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source my-admin \
  --network testnet \
  -- approve \
  --owner "$ADMIN" \
  --spender "$VESTING_ID" \
  --amount 500000
```

### 6. Create a vesting schedule

```bash
START=$(date +%s)
BENEFICIARY="G..."

stellar contract invoke \
  --id "$VESTING_ID" \
  --source my-admin \
  --network testnet \
  -- create_schedule \
  --params "{ \"from\": \"$ADMIN\", \"beneficiary\": \"$BENEFICIARY\", \"token_address\": \"$TOKEN_ID\", \"total_amount\": \"500000\", \"start_time\": $START, \"cliff_duration\": 60, \"total_duration\": 200 }"
```

### 7. Inspect a schedule

Use the `scripts/query-schedule.sh` helper to read any vesting schedule by ID
without writing a custom `stellar contract invoke` command:

```bash
chmod +x scripts/query-schedule.sh

# Pass the contract ID and schedule ID as positional args
./scripts/query-schedule.sh "$VESTING_ID" 1

# Or set the env var and pass only the schedule ID
VESTING_CONTRACT_ID="$VESTING_ID" ./scripts/query-schedule.sh "" 1
```

The script calls `get_schedule` as a simulation, so no signing or XLM fees
are required.

### 8. Claim vested tokens

After 60 seconds (cliff period):

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source my-beneficiary \
  --network testnet \
  -- claim \
  --schedule_id 1
```

### 8. Verify on-chain

- **Stellar Expert** (contract storage):
  `https://stellar.expert/explorer/testnet/contract/<VESTING_ID>`
- **Stellar Lab** (invoke contract):
  `https://lab.stellar.org/smart-contracts/contract-explorer?networkId=testnet&contract=<VESTING_ID>`

## Common pitfalls

These errors were captured by running the walkthrough above against testnet
with stellar-cli 28.0.0, the version CI pins. Contract errors show up as
`Error(Contract, #N)`. For the vesting contract, `N` is the `VestingError` code
in `contracts/vesting/src/lib.rs`. For the token, it's the `TokenError` code in
`contracts/token/src/lib.rs`.

### Unfunded source account

Symptom: the deploy fails on the WASM upload step.

```text
❌ error: Account not found: GCAWJIOWEOSAGSE6I7SEZ45WTJYONF6RQGX5XRD5T5SHR54OWJWGR3HB
```

Cause: the identity exists locally but has never been funded on testnet, so
there is no account to pay fees from. `stellar keys generate` without `--fund`
does this.

Fix: fund it through Friendbot and retry.

```bash
stellar keys fund my-admin --network testnet
```

### Wrong WASM target

Symptom: building with `wasm32-unknown-unknown` (old guides and the old
Makefile used it) fails inside `soroban-sdk`:

```text
Rust compiler 1.82+ with target 'wasm32-unknown-unknown' is unsupported by the Soroban Environment, use 'wasm32v1-none' available with Rust 1.84+.
```

When you deploy from the old output directory, you get:

```text
❌ error: reading file target/wasm32-unknown-unknown/release/vesting.wasm: No such file or directory (os error 2)
```

Fix: install the supported target, build with `stellar contract build`, and
deploy from `target/wasm32v1-none/release/`.

```bash
rustup target add wasm32v1-none
stellar contract build
```

### Missing `--` before constructor or function args

Symptom:

```text
error: unexpected argument '--admin' found

  tip: to pass '--admin' as a value, use '-- --admin'
```

Cause: everything after a bare `--` goes to the contract. Without that
separator, the CLI parses `--admin` as one of its own flags.

Fix: put `--` between the CLI flags and the contract args, as in step 4 of the
walkthrough: `... --network testnet -- --admin "$ADMIN"`.

### Identity and network mismatches

Each of these was triggered by changing a single flag in an otherwise working
command:

| You passed | Error |
|------------|-------|
| `--source` an alias that was never generated | `❌ error: Failed to find config identity for pf-nosuchkey` |
| `--network` misspelled (e.g. `tesnet`) | `❌ error: Failed to find config network for tesnet` |
| `--network futurenet` with a testnet contract ID | `❌ error: Contract not found: CBZT...RXBF` |
| `"from"` in `create_schedule` is not the `--source` account | `❌ error: transaction simulation failed: HostError: Error(Auth, InvalidAction)` |

Fix: check what the CLI knows about, then use the same alias and network that
deployed the contract.

```bash
stellar keys ls
stellar network ls
stellar keys address my-admin   # must match "from" in create_schedule
```

`create_schedule` moves tokens with `token.transfer(from, ...)`, so `from` must
sign the transaction (in this walkthrough it is `ADMIN`) and must hold at least
`total_amount`. The `approve` call in step 5 doesn't affect this path. If
`from` doesn't hold enough tokens, the token contract fails with
`InsufficientBalance`:

```text
❌ error: transaction simulation failed: HostError: Error(Contract, #4)
```

### Schedule parameters and timestamps

| Mistake | Error | Fix |
|---------|-------|-----|
| `total_amount` passed as a bare JSON number | `❌ error: Failed to parse argument 'params': invalid type: number, expected string or map` | `i128` values must be JSON strings: `"total_amount": "500000"` |
| `cliff_duration` greater than `total_duration` | `HostError: Error(Contract, #3)` (`CliffExceedsDuration`) | Keep `cliff_duration <= total_duration` |
| `claim` before `start_time + cliff_duration` | `HostError: Error(Contract, #8)` (`NothingToClaim`) | Wait for the cliff; check with `get_claimable --schedule_id <id>` first |
| Wrong schedule ID | `HostError: Error(Contract, #6)` (`ScheduleNotFound`) | Use the ID that `create_schedule` printed |

`start_time` is in Unix **seconds** (`date +%s`). If you pass milliseconds
(for example `Date.now()` from JavaScript), `create_schedule` still succeeds,
but the schedule starts tens of thousands of years from now. `get_claimable` keeps
returning `"0"`, and every `claim` fails with `Error(Contract, #8)`. The only
fix is to create a new schedule with a seconds-based `start_time`.

Arguments use the contract's parameter names, not `--id`. Use
`get_claimable --schedule_id 1` and `claim --schedule_id 1`. Passing `--id`
prints `error: unexpected argument '--id' found`.

### Expired (archived) storage entries

A testnet contract that hasn't been called for roughly 180 days can have its
instance or persistent entries archived. An archived instance makes every call
fail until you restore it. [docs/ttl.md](ttl.md) explains which keys expire
and when. To restore and then extend the instance TTL:

```bash
stellar contract restore --id "$VESTING_ID" --source my-admin --network testnet
stellar contract invoke --id "$VESTING_ID" --source my-admin --network testnet --send=yes -- extend_ttl
```

`--send=yes` is needed because `extend_ttl` does nothing while the TTL is
still above the bump threshold. In that case the CLI treats the simulation as
read-only, prints `Simulation identified as read-only. Send by rerunning with
--send=yes.`, and doesn't submit a transaction.

Every public entry point already bumps the TTL, so this only affects demo
contracts that have sat idle. Deploying fresh contracts is also a valid fix on
testnet.

## DAO vesting example

A typical DAO team vesting setup:

| Parameter | Value |
|-----------|-------|
| total_amount | 4,000,000 tokens |
| start_time | team TGE date (unix timestamp) |
| cliff_duration | 31,536,000 (1 year in seconds) |
| total_duration | 126,144,000 (4 years in seconds) |

This locks all tokens for 1 year, then releases them linearly over the
following 3 years — a standard DAO contributor vesting schedule.

---

## Multisig governance demo

This section walks through deploying the multisig contract, transferring the
vesting admin role to it, and executing a governance proposal to create a
vesting schedule. It uses a single owner with threshold 1 for simplicity —
replace with a real N-of-M setup for production.

### Prerequisites

Complete steps 1–4 of the manual walkthrough above so you have `TOKEN_ID`,
`VESTING_ID`, `ADMIN`, and `my-admin` set in your shell.

### Step 1 — Deploy the multisig contract

```bash
export MULTISIG_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/multisig.wasm \
  --source my-admin \
  --network testnet \
  -- \
  --owners "[\"$ADMIN\"]" \
  --threshold 1)
echo "Multisig: $MULTISIG_ID"
```

The `--owners` arg is a JSON array of Stellar account addresses. Threshold 1
means any single owner can confirm and execute a proposal. For a 2-of-3
setup, pass three addresses and `--threshold 2`.

### Step 2 — Transfer vesting admin to the multisig

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source my-admin \
  --network testnet \
  -- transfer_admin \
  --new_admin "$MULTISIG_ID"
```

After this call, `ADMIN` can no longer call `create_schedule`, `revoke`, or
`pause` directly — only proposals executed by the multisig can do so.

> **Confirm the change:**
> ```bash
> stellar contract invoke \
>   --id "$VESTING_ID" \
>   --source my-admin \
>   --network testnet \
>   -- get_admin
> ```
> The output should be the multisig contract address.

### Step 3 — Mint tokens and approve the vesting contract

The token admin is still `ADMIN`, so minting and approving still use `ADMIN`
directly:

```bash
# Mint 1 000 000 tokens to the admin (will fund the schedule)
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source my-admin \
  --network testnet \
  -- mint \
  --to "$ADMIN" \
  --amount 1000000

# Approve the vesting contract to pull the tokens
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source my-admin \
  --network testnet \
  -- approve \
  --owner "$ADMIN" \
  --spender "$VESTING_ID" \
  --amount 500000
```

### Step 4 — Submit a create_schedule proposal

Encode the `create_schedule` parameters as a JSON object and submit a
`ProposalAction::Call` proposal. The `--args` field must be a JSON-serialised
`Vec<Val>` matching the single `params` argument of `create_schedule`.

```bash
START=$(date +%s)
BENEFICIARY="G..."  # replace with a real Stellar account address

stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source my-admin \
  --network testnet \
  -- submit \
  --proposer "$ADMIN" \
  --action "{ \"Call\": { \"target\": \"$VESTING_ID\", \"function\": \"create_schedule\", \"args\": [{ \"from\": \"$ADMIN\", \"beneficiary\": \"$BENEFICIARY\", \"token_address\": \"$TOKEN_ID\", \"total_amount\": 500000, \"start_time\": $START, \"cliff_duration\": 60, \"total_duration\": 200 }] } }" \
  --description '"Create schedule for beneficiary"'
```

`submit` returns the proposal ID (a `u64`). Save it:

```bash
export PROPOSAL_ID=1   # use the value returned by submit
```

### Step 5 — Confirm the proposal

```bash
stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source my-admin \
  --network testnet \
  -- confirm \
  --owner "$ADMIN" \
  --proposal_id "$PROPOSAL_ID"
```

With threshold 1 and one confirmation, the proposal is now executable. For a
2-of-3 multisig you would repeat this step with two different owner keys.

### Step 6 — Execute the proposal

`execute` is permissionless — any account can call it once threshold is met:

```bash
stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source my-admin \
  --network testnet \
  -- execute \
  --proposal_id "$PROPOSAL_ID"
```

The multisig calls `vesting.create_schedule(params)` as the vesting admin. The
cross-contract call satisfies `admin.require_auth()` inside the vesting
contract because the call originates from the multisig, which is now the admin.

### Step 7 — Verify the schedule was created

```bash
# Check the proposal status (should be Executed)
stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source my-admin \
  --network testnet \
  -- get_proposal \
  --proposal_id "$PROPOSAL_ID"

# Confirm the vesting schedule exists
stellar contract invoke \
  --id "$VESTING_ID" \
  --source my-admin \
  --network testnet \
  -- get_schedule \
  --id 1

# Check claimable amount (0 before cliff, > 0 after)
stellar contract invoke \
  --id "$VESTING_ID" \
  --source my-admin \
  --network testnet \
  -- get_claimable \
  --id 1
```

After 60 seconds (the cliff), the beneficiary can claim:

```bash
stellar contract invoke \
  --id "$VESTING_ID" \
  --source my-beneficiary \
  --network testnet \
  -- claim \
  --schedule_id 1
```

### Operating the multisig with `scripts/invoke-multisig.sh`

Instead of typing the full `stellar contract invoke` argument syntax for every
multisig operation, use the [`scripts/invoke-multisig.sh`](../scripts/invoke-multisig.sh)
helper, which wraps `submit`, `confirm`, and `execute`:

```bash
export MULTISIG_CONTRACT_ID=CABC…XYZ
export SOURCE_ACCOUNT=my-admin   # fallback signer for `execute`

# 1. Proposer submits a create_schedule proposal (proposer signs)
./scripts/invoke-multisig.sh submit GABC…PROPOSER \
  '{"Call":{"target":"'$VESTING_ID'","function":"create_schedule","args":[...]}}' \
  'Create vesting schedule'

# 2. Owner confirms the proposal (owner signs)
./scripts/invoke-multisig.sh confirm GABC…OWNER 1

# 3. Anyone executes once threshold is met
./scripts/invoke-multisig.sh execute 1
```

See the script header for the full usage and environment variables.

### Next steps

- Add more owners and raise the threshold for a real N-of-M governance setup
- Submit `ProposalAction::AddOwner` / `RemoveOwner` proposals to manage the
  owner set (no direct public entry points exist — all owner changes go through
  proposals)
- Use `revoke_confirmation` to withdraw a confirmation before execution
- Explore `ProposalAction::UpdateThreshold` to change the required vote count

---

## Node.js SDK example: query claimable amount

The script `scripts/examples/query-claimable.js` shows how to call
`get_claimable` directly from Node.js using `@stellar/stellar-sdk`, without
going through the backend REST API.  It is useful for developers building their
own integrations.

```bash
# Install dependencies (run once from repo root)
cd backend && npm ci && cd ..

# Query schedule 1 on the deployed testnet contract
VESTING_CONTRACT_ID=CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF \
  node scripts/examples/query-claimable.js

# Override schedule ID and RPC URL
VESTING_CONTRACT_ID=<your-contract> \
SCHEDULE_ID=3 \
STELLAR_RPC_URL=https://soroban-testnet.stellar.org \
  node scripts/examples/query-claimable.js
```

The script simulates the contract call (no signing or fees required) and prints
the claimable token amount as a BigInt value.

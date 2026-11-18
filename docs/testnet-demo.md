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
  --params "{ \"from\": \"$ADMIN\", \"beneficiary\": \"$BENEFICIARY\", \"token_address\": \"$TOKEN_ID\", \"total_amount\": 500000, \"start_time\": $START, \"cliff_duration\": 60, \"total_duration\": 200 }"
```

### 7. Claim vested tokens

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

### Next steps

- Add more owners and raise the threshold for a real N-of-M governance setup
- Submit `ProposalAction::AddOwner` / `RemoveOwner` proposals to manage the
  owner set (no direct public entry points exist — all owner changes go through
  proposals)
- Use `revoke_confirmation` to withdraw a confirmation before execution
- Explore `ProposalAction::UpdateThreshold` to change the required vote count

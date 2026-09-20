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
4. Deploy the token and vesting contracts
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
  --network testnet)
echo "Token: $TOKEN_ID"
```

Initialize it:

```bash
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source my-admin \
  --network testnet \
  -- initialize \
  --admin "$ADMIN" \
  --name '"My Token"' \
  --symbol '"MTK"' \
  --decimals 7 \
  --initial_supply 0
```

### 4. Deploy the vesting contract

```bash
export VESTING_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/vesting.wasm \
  --source my-admin \
  --network testnet)
echo "Vesting: $VESTING_ID"

stellar contract invoke \
  --id "$VESTING_ID" \
  --source my-admin \
  --network testnet \
  -- initialize \
  --admin "$ADMIN"
```

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
#!/usr/bin/env bash
# =============================================================================
# demo-testnet.sh — Full vesting lifecycle demo on Stellar Testnet
#
# This script demonstrates the complete flow:
#   1. Fund a new admin account via Friendbot
#   2. Deploy the token and vesting contracts
#   3. Mint tokens into the vesting contract
#   4. Create a vesting schedule for a beneficiary
#   5. Wait for cliff to pass (simulated with --time-bump)
#   6. Claim vested tokens
#   7. Print final balances
#
# Usage:
#   ./scripts/demo-testnet.sh
#
# Prerequisites:
#   - stellar-cli installed (https://install.stellar.org)
#   - No additional tools needed; a fresh keypair is generated automatically
# =============================================================================

set -euo pipefail

NETWORK="testnet"
RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

echo "================================================"
echo " Soroban Vesting Platform — Testnet Demo"
echo "================================================"
echo ""

# ── Step 1: Generate and fund admin keypair ──────────────────────────────────
echo "[1/7] Generating admin keypair..."
ADMIN_SECRET=$(stellar keys generate demo-admin --network testnet 2>&1 | grep 'Secret key' | awk '{print $3}' || stellar keys show demo-admin --secret 2>/dev/null || true)

# Use stellar keys to fund via friendbot
stellar keys fund demo-admin --network testnet
ADMIN_ADDRESS=$(stellar keys address demo-admin)
echo "     Admin address : $ADMIN_ADDRESS"
echo ""

# ── Step 2: Generate beneficiary keypair ─────────────────────────────────────
echo "[2/7] Generating beneficiary keypair..."
stellar keys generate demo-beneficiary --network testnet --fund
BENEFICIARY_ADDRESS=$(stellar keys address demo-beneficiary)
echo "     Beneficiary  : $BENEFICIARY_ADDRESS"
echo ""

# ── Step 3: Build contracts ──────────────────────────────────────────────────
echo "[3/7] Building contracts..."
stellar contract build
echo "     Build complete."
echo ""

# ── Step 4: Deploy token contract ────────────────────────────────────────────
echo "[4/7] Deploying token contract..."
TOKEN_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/token.wasm \
  --source demo-admin \
  --network testnet)
echo "     Token contract : $TOKEN_ID"

# Initialize token
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source demo-admin \
  --network testnet \
  -- initialize \
  --admin "$ADMIN_ADDRESS" \
  --name '"Vesting Token"' \
  --symbol '"VEST"' \
  --decimals 7 \
  --initial_supply 0
echo "     Token initialized."
echo ""

# ── Step 5: Deploy vesting contract ──────────────────────────────────────────
echo "[5/7] Deploying vesting contract..."
VESTING_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/vesting.wasm \
  --source demo-admin \
  --network testnet)
echo "     Vesting contract : $VESTING_ID"

# Initialize vesting
stellar contract invoke \
  --id "$VESTING_ID" \
  --source demo-admin \
  --network testnet \
  -- initialize \
  --admin "$ADMIN_ADDRESS"
echo "     Vesting initialized."
echo ""

# ── Step 6: Mint tokens and approve vesting contract ─────────────────────────
echo "[6/7] Minting tokens and creating vesting schedule..."

# Mint 1,000,000 VEST to admin
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source demo-admin \
  --network testnet \
  -- mint \
  --to "$ADMIN_ADDRESS" \
  --amount 1000000

# Approve vesting contract to pull tokens
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source demo-admin \
  --network testnet \
  -- approve \
  --owner "$ADMIN_ADDRESS" \
  --spender "$VESTING_ID" \
  --amount 500000

# Get current ledger timestamp
NOW=$(stellar ledger --network testnet | jq -r '.sequence')
START_TIME=$(date +%s)

# Create schedule: 500,000 VEST vesting over 200 seconds with 60s cliff
stellar contract invoke \
  --id "$VESTING_ID" \
  --source demo-admin \
  --network testnet \
  -- create_schedule \
  --params "{ \"from\": \"$ADMIN_ADDRESS\", \"beneficiary\": \"$BENEFICIARY_ADDRESS\", \"token_address\": \"$TOKEN_ID\", \"total_amount\": 500000, \"start_time\": $START_TIME, \"cliff_duration\": 60, \"total_duration\": 200 }"

SCHEDULE_ID=1
echo "     Schedule created (ID: $SCHEDULE_ID)"
echo ""

# ── Step 7: Check claimable and claim ─────────────────────────────────────────
echo "[7/7] Checking claimable amount (may be 0 if before cliff)..."
CLAIMABLE=$(stellar contract invoke \
  --id "$VESTING_ID" \
  --source demo-admin \
  --network testnet \
  -- get_claimable \
  --schedule_id $SCHEDULE_ID)
echo "     Claimable now : $CLAIMABLE VEST"

if [ "$CLAIMABLE" -gt 0 ] 2>/dev/null; then
  echo "     Claiming..."
  stellar contract invoke \
    --id "$VESTING_ID" \
    --source demo-beneficiary \
    --network testnet \
    -- claim \
    --schedule_id $SCHEDULE_ID
  echo "     Claimed $CLAIMABLE VEST to $BENEFICIARY_ADDRESS"
else
  echo "     Nothing claimable yet (cliff or start not reached)."
  echo "     Re-run this script after the cliff period to claim."
fi

echo ""
echo "================================================"
echo " Demo complete!"
echo "================================================"
echo ""
echo " Token contract ID   : $TOKEN_ID"
echo " Vesting contract ID : $VESTING_ID"
echo " Schedule ID         : $SCHEDULE_ID"
echo " Admin address       : $ADMIN_ADDRESS"
echo " Beneficiary address : $BENEFICIARY_ADDRESS"
echo ""
echo " View on Stellar Expert:"
echo " https://stellar.expert/explorer/testnet/contract/$VESTING_ID"
echo ""
echo " Inspect on Stellar Lab:"
echo " https://lab.stellar.org/smart-contracts/contract-explorer?networkId=testnet&contract=$VESTING_ID"
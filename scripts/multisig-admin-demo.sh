#!/usr/bin/env bash
# =============================================================================
# multisig-admin-demo.sh — Full multisig admin flow demo on Stellar Testnet
#
# This script demonstrates the complete multisig governance flow:
#   1. Deploy a 1-of-1 multisig contract (single-key, suitable for testing)
#   2. Transfer vesting admin rights to the multisig contract
#   3. Submit a pause() proposal through the multisig
#   4. Confirm the proposal (owner is the same key, threshold = 1)
#   5. Execute the confirmed proposal
#   6. Verify the vesting contract is now paused
#
# Usage:
#   # With pre-deployed contracts (reads from .env or shell environment):
#   export TOKEN_ID=<token contract ID>
#   export VESTING_ID=<vesting contract ID>
#   ./scripts/multisig-admin-demo.sh
#
#   # Or pass IDs as arguments:
#   ./scripts/multisig-admin-demo.sh <TOKEN_ID> <VESTING_ID>
#
# Prerequisites:
#   - stellar-cli installed (https://install.stellar.org)
#   - jq installed (apt install jq / brew install jq)
#   - Token and vesting contracts already deployed on Testnet
#     (run scripts/demo-testnet.sh first, or set the env vars above)
#   - The deploying account (demo-admin key) is already funded
#     If it doesn't exist yet: stellar keys generate demo-admin --network testnet --fund
#
# Environment variables (all optional if passed as arguments):
#   TOKEN_ID   — deployed token contract ID
#   VESTING_ID — deployed vesting contract ID
# =============================================================================

set -euo pipefail

NETWORK="testnet"

# ── Resolve contract IDs ──────────────────────────────────────────────────────
TOKEN_ID="${1:-${TOKEN_ID:-}}"
VESTING_ID="${2:-${VESTING_ID:-}}"

if [[ -z "$TOKEN_ID" || -z "$VESTING_ID" ]]; then
  echo "ERROR: TOKEN_ID and VESTING_ID must be set."
  echo "       Run scripts/demo-testnet.sh first, or export the variables:"
  echo "         export TOKEN_ID=<id>"
  echo "         export VESTING_ID=<id>"
  echo "       Or pass them as arguments: $0 <TOKEN_ID> <VESTING_ID>"
  exit 1
fi

ADMIN_ADDRESS=$(stellar keys address demo-admin)

echo "================================================"
echo " Soroban Vesting Platform — Multisig Admin Demo"
echo "================================================"
echo ""
echo " Admin address  : $ADMIN_ADDRESS"
echo " Token contract : $TOKEN_ID"
echo " Vesting contract: $VESTING_ID"
echo ""

# ── Step 1: Deploy 1-of-1 multisig ───────────────────────────────────────────
echo "[1/6] Deploying multisig contract (1-of-1)..."
MULTISIG_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/multisig.wasm \
  --source demo-admin \
  --network "$NETWORK" \
  -- \
  --owners "[\"$ADMIN_ADDRESS\"]" \
  --threshold 1)
echo "      Multisig contract : $MULTISIG_ID"
echo ""

# ── Step 2: Transfer vesting admin to the multisig ───────────────────────────
echo "[2/6] Transferring vesting admin to the multisig..."
# transfer_admin requires auth from both the current admin (demo-admin)
# AND the new admin (multisig). For the multisig to accept the admin role,
# we call transfer_admin with the current admin as source.
# The multisig contract address becomes the new admin.
stellar contract invoke \
  --id "$VESTING_ID" \
  --source demo-admin \
  --network "$NETWORK" \
  -- transfer_admin \
  --new_admin "$MULTISIG_ID"
echo "      Vesting admin is now: $MULTISIG_ID"
echo ""

# ── Step 3: Submit a pause() proposal through the multisig ───────────────────
echo "[3/6] Submitting a pause() proposal via multisig..."
# ProposalAction::Call encodes a cross-contract call:
#   target        = vesting contract ID
#   function_name = "pause"
#   args          = [] (pause takes no arguments beyond Env)
#
# The action JSON must be encoded as a Soroban-compatible value.
# Using stellar contract invoke, the action is passed as a JSON struct
# matching the ProposalAction::Call(CallData) variant.
PROPOSAL_OUTPUT=$(stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source demo-admin \
  --network "$NETWORK" \
  -- submit \
  --proposer "$ADMIN_ADDRESS" \
  --action "{\"Call\":{\"target\":\"$VESTING_ID\",\"function_name\":\"pause\",\"args\":[]}}" \
  --description '"Pause vesting contract via multisig governance"')

# The return value is the new proposal ID (u64)
PROPOSAL_ID=$(echo "$PROPOSAL_OUTPUT" | tr -d '"')
echo "      Proposal ID : $PROPOSAL_ID"
echo ""

# ── Step 4: Confirm the proposal ─────────────────────────────────────────────
echo "[4/6] Confirming proposal $PROPOSAL_ID (owner: $ADMIN_ADDRESS)..."
stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source demo-admin \
  --network "$NETWORK" \
  -- confirm \
  --owner "$ADMIN_ADDRESS" \
  --proposal_id "$PROPOSAL_ID"
echo "      Confirmed."
echo ""

# ── Step 5: Execute the proposal ─────────────────────────────────────────────
echo "[5/6] Executing proposal $PROPOSAL_ID (permissionless once threshold met)..."
stellar contract invoke \
  --id "$MULTISIG_ID" \
  --source demo-admin \
  --network "$NETWORK" \
  -- execute \
  --proposal_id "$PROPOSAL_ID"
echo "      Executed."
echo ""

# ── Step 6: Verify vesting contract is paused ────────────────────────────────
echo "[6/6] Verifying vesting contract is paused..."
IS_PAUSED=$(stellar contract invoke \
  --id "$VESTING_ID" \
  --source demo-admin \
  --network "$NETWORK" \
  -- is_paused)

if [[ "$IS_PAUSED" == "true" ]]; then
  echo "      ✓ Vesting contract is PAUSED — multisig governance succeeded!"
else
  echo "      ✗ Unexpected: is_paused returned '$IS_PAUSED'"
  exit 1
fi

echo ""
echo "================================================"
echo " Multisig admin demo complete!"
echo "================================================"
echo ""
echo " Token contract ID    : $TOKEN_ID"
echo " Vesting contract ID  : $VESTING_ID"
echo " Multisig contract ID : $MULTISIG_ID"
echo " Admin address        : $ADMIN_ADDRESS"
echo " Executed proposal ID : $PROPOSAL_ID"
echo ""
echo " The vesting contract is now paused and governed by the multisig."
echo " To unpause, submit a new proposal with function_name=\"unpause\"."
echo ""
echo " View on Stellar Expert:"
echo " https://stellar.expert/explorer/testnet/contract/$MULTISIG_ID"
echo " https://stellar.expert/explorer/testnet/contract/$VESTING_ID"

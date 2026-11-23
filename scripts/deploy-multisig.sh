#!/usr/bin/env bash
# =============================================================================
# deploy-multisig.sh — Deploy the multisig contract to Stellar Testnet
#
# Builds the multisig WASM (if needed) and deploys it with configurable
# owners and confirmation threshold. The contract constructor is invoked
# at deploy time — no separate initialise step is required.
#
# Usage:
#   OWNERS='["G...","G..."]' THRESHOLD=2 ./scripts/deploy-multisig.sh
#
# Environment variables:
#   OWNERS      — JSON array of Stellar account addresses (G…) that form the
#                 initial owner set.  At least one owner is required.
#                 Example: '["GABC...","GDEF..."]'
#                 Default: the address of SOURCE_ACCOUNT.
#
#   THRESHOLD   — Number of owner confirmations required to execute a
#                 proposal.  Must be between 1 and len(OWNERS) inclusive.
#                 Default: 1
#
#   SOURCE_ACCOUNT — stellar-cli identity alias (or secret key) used to pay
#                    deployment fees and sign the transaction.
#                    Read from .env if not already set.
#                    Default: "my-admin"
#
#   NETWORK     — Stellar network to deploy to ("testnet" or "mainnet").
#                 Default: "testnet"
#
#   SKIP_BUILD  — Set to "1" to skip `stellar contract build` (use an
#                 existing WASM at target/wasm32v1-none/release/multisig.wasm).
#                 Default: unset (build runs every time)
#
# Prerequisites:
#   - Rust + wasm32v1-none target  (rustup target add wasm32v1-none)
#   - stellar-cli >= 20.0.0        (https://install.stellar.org)
#   - jq (optional)                — used to persist the contract ID to
#                                    deployments.json
#
# Example — deploy a 2-of-3 multisig:
#   export OWNERS='["GAAA...","GBBB...","GCCC..."]'
#   export THRESHOLD=2
#   export SOURCE_ACCOUNT=my-admin
#   ./scripts/deploy-multisig.sh
#
# Example — single-owner testnet demo (threshold 1):
#   ADMIN=$(stellar keys address my-admin)
#   OWNERS="[\"$ADMIN\"]" THRESHOLD=1 ./scripts/deploy-multisig.sh
# =============================================================================

set -euo pipefail

# ── Load .env (optional) ─────────────────────────────────────────────────────
if [ -f ".env" ]; then
  # shellcheck disable=SC1091
  set -o allexport && source .env && set +o allexport
fi

# ── Configuration ─────────────────────────────────────────────────────────────
NETWORK="${NETWORK:-testnet}"
SKIP_BUILD="${SKIP_BUILD:-0}"
SOURCE="${SOURCE_ACCOUNT:-my-admin}"
THRESHOLD="${THRESHOLD:-1}"
WASM="target/wasm32v1-none/release/multisig.wasm"
DEPLOYMENTS="deployments.json"

# Default OWNERS to a single-element array containing the source account address
# when no owners are explicitly provided.
if [ -z "${OWNERS:-}" ]; then
  ADMIN_ADDR=$(stellar keys address "$SOURCE" 2>/dev/null || true)
  if [ -z "$ADMIN_ADDR" ]; then
    echo "✗ OWNERS is not set and could not resolve address for identity '${SOURCE}'."
    echo "  Set OWNERS='[\"G...\"]' or ensure '${SOURCE}' is a valid stellar-cli identity."
    exit 1
  fi
  OWNERS="[\"${ADMIN_ADDR}\"]"
  echo "  OWNERS not set — defaulting to source account address: $ADMIN_ADDR"
fi

RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
if [ "$NETWORK" = "mainnet" ]; then
  RPC_URL="https://soroban-mainnet.stellar.org"
  NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
fi

echo "================================================"
echo " Soroban Multisig — Deploy"
echo "================================================"
echo "  Network   : $NETWORK"
echo "  Source    : $SOURCE"
echo "  Threshold : $THRESHOLD"
echo "  Owners    : $OWNERS"
echo ""

# ── Build ─────────────────────────────────────────────────────────────────────
if [ "$SKIP_BUILD" = "1" ]; then
  echo "▶ Skipping build (SKIP_BUILD=1)"
else
  echo "▶ Building multisig contract…"
  stellar contract build
  echo "  Build complete."
fi

if [ ! -f "$WASM" ]; then
  echo "✗ WASM not found at $WASM"
  echo "  Run 'stellar contract build' first, or unset SKIP_BUILD."
  exit 1
fi

echo ""

# ── Deploy ────────────────────────────────────────────────────────────────────
echo "▶ Deploying multisig to ${NETWORK}…"

MULTISIG_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$SOURCE" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- \
  --owners "$OWNERS" \
  --threshold "$THRESHOLD")

echo ""
echo "✓ Multisig deployed: $MULTISIG_ID"
echo ""
echo "  Owners    : $OWNERS"
echo "  Threshold : $THRESHOLD"
echo ""

# ── Persist to deployments.json ──────────────────────────────────────────────
if command -v jq &>/dev/null; then
  if [ ! -f "$DEPLOYMENTS" ]; then echo '{}' > "$DEPLOYMENTS"; fi
  tmp=$(mktemp)
  jq --arg k "multisig_${NETWORK}" --arg v "$MULTISIG_ID" '. + {($k): $v}' \
    "$DEPLOYMENTS" > "$tmp" && mv "$tmp" "$DEPLOYMENTS"
  echo "✓ Saved to ${DEPLOYMENTS} (key: multisig_${NETWORK})"
else
  echo "  (install jq to auto-save to ${DEPLOYMENTS})"
  echo "  multisig_${NETWORK}=${MULTISIG_ID}"
fi

echo ""
echo "================================================"
echo " Next steps"
echo "================================================"
echo ""
echo "  Transfer vesting admin to this multisig:"
echo "    stellar contract invoke \\"
echo "      --id \"\$VESTING_ID\" --source $SOURCE --network $NETWORK \\"
echo "      -- transfer_admin --new_admin $MULTISIG_ID"
echo ""
echo "  View on Stellar Expert:"
echo "  https://stellar.expert/explorer/${NETWORK}/contract/${MULTISIG_ID}"
echo ""
echo "  See docs/testnet-demo.md for the full multisig governance walkthrough."

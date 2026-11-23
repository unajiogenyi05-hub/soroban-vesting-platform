#!/usr/bin/env bash
# deploy.sh  — Build and deploy the vesting or token contract to Stellar
#
# Usage:
#   ./scripts/deploy.sh <contract-name> [network]
#
# Supported contracts: vesting, token
# Network defaults to testnet.
#
# Required environment variables (set in .env or the calling shell):
#   SOURCE_ACCOUNT — Stellar CLI key name or secret used to sign the deploy tx
#   ADMIN          — Stellar address set as the contract admin at deploy time
#
# Additional variables for the token contract (optional, defaults shown):
#   TOKEN_NAME     — token name     (default: "Vesting Token")
#   TOKEN_SYMBOL   — token symbol   (default: "VEST")
#   TOKEN_DECIMALS — decimal places (default: 7)
#
# Examples:
#   ./scripts/deploy.sh vesting testnet
#   ./scripts/deploy.sh token testnet
#
# See docs/testnet-demo.md for a full step-by-step walkthrough.

set -euo pipefail

CONTRACT="${1:?Usage: deploy.sh <contract-name> [network]}"
NETWORK="${2:-testnet}"

WASM="target/wasm32v1-none/release/${CONTRACT}.wasm"
DEPLOYMENTS="deployments.json"

# Load env
if [ -f ".env" ]; then
  # shellcheck disable=SC1091
  set -o allexport && source .env && set +o allexport
fi

SOURCE="${SOURCE_ACCOUNT:?Set SOURCE_ACCOUNT in .env or environment}"
ADMIN_ADDR="${ADMIN:?Set ADMIN in .env or environment (Stellar G... address)}"

RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
if [ "$NETWORK" = "mainnet" ]; then
  RPC_URL="https://soroban-mainnet.stellar.org"
  NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
fi

echo "▶ Building ${CONTRACT}…"
stellar contract build

if [ ! -f "$WASM" ]; then
  echo "✗ WASM not found at $WASM"
  exit 1
fi

echo "▶ Deploying ${CONTRACT} to ${NETWORK}…"

# Build the constructor-argument list for each supported contract.
# Constructor args are passed after the -- separator.
case "$CONTRACT" in
  vesting)
    CONSTRUCTOR_ARGS=(-- --admin "$ADMIN_ADDR")
    ;;
  token)
    TOKEN_NAME="${TOKEN_NAME:-Vesting Token}"
    TOKEN_SYMBOL="${TOKEN_SYMBOL:-VEST}"
    TOKEN_DECIMALS="${TOKEN_DECIMALS:-7}"
    CONSTRUCTOR_ARGS=(-- --admin "$ADMIN_ADDR" --name "\"${TOKEN_NAME}\"" --symbol "\"${TOKEN_SYMBOL}\"" --decimals "$TOKEN_DECIMALS" --initial_supply 0)
    ;;
  *)
    echo "✗ Unknown contract '${CONTRACT}'. Supported: vesting, token"
    echo "  Pass constructor args manually after a -- separator:"
    echo "  ./scripts/deploy.sh <name> <network> -- <args...>"
    exit 1
    ;;
esac

CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$SOURCE" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  "${CONSTRUCTOR_ARGS[@]}")

echo "✓ Deployed: ${CONTRACT_ID}"

# Persist to deployments.json
if command -v jq &>/dev/null; then
  if [ ! -f "$DEPLOYMENTS" ]; then echo '{}' > "$DEPLOYMENTS"; fi
  tmp=$(mktemp)
  jq --arg k "${CONTRACT}_${NETWORK}" --arg v "$CONTRACT_ID" '. + {($k): $v}' \
    "$DEPLOYMENTS" > "$tmp" && mv "$tmp" "$DEPLOYMENTS"
  echo "✓ Saved to ${DEPLOYMENTS}"
else
  echo "  (install jq to auto-save to ${DEPLOYMENTS})"
  echo "  ${CONTRACT}_${NETWORK}=${CONTRACT_ID}"
fi

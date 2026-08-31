#!/usr/bin/env bash
# deploy.sh  — Build and deploy a single Soroban contract
# Usage: ./scripts/deploy.sh <contract-name> <network>
# Example: ./scripts/deploy.sh vesting testnet

set -euo pipefail

CONTRACT="${1:?Usage: deploy.sh <contract-name> <network>}"
NETWORK="${2:-testnet}"

WASM="target/wasm32-unknown-unknown/release/${CONTRACT}.wasm"
DEPLOYMENTS="deployments.json"

# Load env
if [ -f ".env" ]; then
  # shellcheck disable=SC1091
  set -o allexport && source .env && set +o allexport
fi

SOURCE="${SOURCE_ACCOUNT:?Set SOURCE_ACCOUNT in .env or environment}"

RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
if [ "$NETWORK" = "mainnet" ]; then
  RPC_URL="https://soroban-mainnet.stellar.org"
  NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
fi

echo "▶ Building ${CONTRACT}…"
cargo build -p "${CONTRACT}" --target wasm32-unknown-unknown --release

if [ ! -f "$WASM" ]; then
  echo "✗ WASM not found at $WASM"
  exit 1
fi

echo "▶ Deploying ${CONTRACT} to ${NETWORK}…"
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$SOURCE" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE")

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

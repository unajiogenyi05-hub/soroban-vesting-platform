#!/usr/bin/env bash
# invoke.sh — Call a function on a deployed Soroban contract
# Usage: ./scripts/invoke.sh <contract-id> <function> <network> [-- <args...>]
# Example: ./scripts/invoke.sh CABC…XYZ claim testnet -- --schedule_id 1

set -euo pipefail

CONTRACT_ID="${1:?Usage: invoke.sh <contract-id> <function> <network>}"
FUNCTION="${2:?Provide a function name}"
NETWORK="${3:-testnet}"
shift 3

if [ -f ".env" ]; then
  set -o allexport && source .env && set +o allexport
fi

SOURCE="${SOURCE_ACCOUNT:?Set SOURCE_ACCOUNT in .env}"

RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
if [ "$NETWORK" = "mainnet" ]; then
  RPC_URL="https://soroban-mainnet.stellar.org"
  NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
fi

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOURCE" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- "$FUNCTION" "$@"

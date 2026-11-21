#!/usr/bin/env bash
# query-token.sh — Query a Soroban SEP-0041 token balance and total supply
# Usage: ./scripts/query-token.sh <TOKEN_CONTRACT_ID> <ADDRESS> [--network testnet|mainnet]
# Example: ./scripts/query-token.sh CABC…XYZ GD…XYZ testnet
#
# Queries:
#   balance(<ADDRESS>)   — token balance of the given account
#   total_supply()       — total supply of the token contract
#
# Requires SOURCE_ACCOUNT to be set in .env or the environment.

set -euo pipefail

TOKEN_CONTRACT_ID="${1:?Usage: query-token.sh <token-contract-id> <address> [--network testnet|mainnet]}"
ADDRESS="${2:?Provide a token account address}"
NETWORK="${3:-testnet}"

if [ -f ".env" ]; then
  set -o allexport && source .env && set +o allexport
fi

SOURCE="${SOURCE_ACCOUNT:?Set SOURCE_ACCOUNT in .env or export it}"

RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
if [ "$NETWORK" = "mainnet" ]; then
  RPC_URL="https://soroban-mainnet.stellar.org"
  NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
fi

echo "=== Balance of $ADDRESS ==="
stellar contract invoke \
  --id "$TOKEN_CONTRACT_ID" \
  --source "$SOURCE" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- balance --account "$ADDRESS"

echo ""
echo "=== Total Supply ==="
stellar contract invoke \
  --id "$TOKEN_CONTRACT_ID" \
  --source "$SOURCE" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- total_supply

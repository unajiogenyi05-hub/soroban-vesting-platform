#!/usr/bin/env bash
# Generate TypeScript bindings using stellar contract bindings typescript (#107)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
DEPLOYMENTS_FILE="$ROOT_DIR/deployments.json"
NETWORK="${STELLAR_NETWORK:-testnet}"
OUTPUT_DIR="${1:-$ROOT_DIR/sdk}"

mkdir -p "$OUTPUT_DIR"

echo "Regenerating TypeScript bindings for network: $NETWORK"
echo "Output directory: $OUTPUT_DIR"

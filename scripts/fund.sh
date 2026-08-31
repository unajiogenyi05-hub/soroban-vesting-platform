#!/usr/bin/env bash
# fund.sh — Fund an account on Stellar testnet via Friendbot
# Usage: ./scripts/fund.sh [public-key]

set -euo pipefail

if [ -f ".env" ]; then
  set -o allexport && source .env && set +o allexport
fi

ACCOUNT="${1:-${SOURCE_ACCOUNT:-}}"

if [ -z "$ACCOUNT" ]; then
  echo "Usage: fund.sh <public-key>  or set SOURCE_ACCOUNT in .env"
  exit 1
fi

echo "▶ Funding ${ACCOUNT} on testnet…"
curl -s "https://friendbot.stellar.org/?addr=${ACCOUNT}" | \
  python3 -c "import sys,json; d=json.load(sys.stdin); print('✓ tx:', d.get('hash','ok'))" \
  2>/dev/null || echo "✓ Request sent"

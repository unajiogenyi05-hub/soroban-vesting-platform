#!/usr/bin/env bash
# =============================================================================
# query-schedule.sh — Read a vesting schedule from testnet by ID
#
# Calls get_schedule(schedule_id) on the vesting contract as a read-only
# simulation (no signing or fees required) and pretty-prints the result.
#
# Usage:
#   ./scripts/query-schedule.sh <VESTING_CONTRACT_ID> <SCHEDULE_ID>
#
# Examples:
#   # Query schedule 1 on the deployed testnet contract
#   ./scripts/query-schedule.sh \
#     CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF \
#     1
#
#   # Use the VESTING_CONTRACT_ID env var and pass only the schedule ID
#   VESTING_CONTRACT_ID=CDBSJE... ./scripts/query-schedule.sh "" 2
#
# Environment variables (all optional):
#   VESTING_CONTRACT_ID  — overrides positional arg $1 when $1 is empty
#   STELLAR_NETWORK      — "testnet" (default) or "mainnet"
#   SOURCE_ACCOUNT       — source account alias or key (default: a random
#                          read-only account; override if your CLI requires one)
#
# Prerequisites:
#   stellar-cli installed: curl -sSfL https://install.stellar.org | sh
#
# Notes:
#   get_schedule is a pure read-only getter — it carries no write footprint.
#   stellar-cli submits it as a simulation, so the caller pays no XLM fees
#   and does not need a funded account with a secret key.
# =============================================================================

set -euo pipefail

# ── Resolve arguments ──────────────────────────────────────────────────────────

CONTRACT_ID="${1:-${VESTING_CONTRACT_ID:-}}"
SCHEDULE_ID="${2:-}"

if [ -z "$CONTRACT_ID" ]; then
  echo "Error: VESTING_CONTRACT_ID is required." >&2
  echo "Usage: $0 <VESTING_CONTRACT_ID> <SCHEDULE_ID>" >&2
  exit 1
fi

if [ -z "$SCHEDULE_ID" ]; then
  echo "Error: SCHEDULE_ID is required." >&2
  echo "Usage: $0 <VESTING_CONTRACT_ID> <SCHEDULE_ID>" >&2
  exit 1
fi

NETWORK="${STELLAR_NETWORK:-testnet}"

# ── Load .env if present (non-fatal) ─────────────────────────────────────────

if [ -f ".env" ]; then
  set -o allexport && source .env && set +o allexport
fi

# ── Invoke ────────────────────────────────────────────────────────────────────

echo "Network  : $NETWORK"
echo "Contract : $CONTRACT_ID"
echo "Schedule : $SCHEDULE_ID"
echo ""

stellar contract invoke \
  --id    "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- get_schedule \
  --schedule_id "$SCHEDULE_ID"

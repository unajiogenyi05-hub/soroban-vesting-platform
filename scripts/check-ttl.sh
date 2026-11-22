#!/usr/bin/env bash
# =============================================================================
# check-ttl.sh — Query remaining TTL for deployed contracts and warn when low
#
# Uses `stellar contract read` to fetch the current TTL for the instance
# storage entry of each deployed contract (token, vesting, multisig).
# Prints a warning when any contract's TTL falls below the
# INSTANCE_BUMP_THRESHOLD (~30 days / 518 400 ledgers).
#
# Usage:
#   ./scripts/check-ttl.sh
#
# Environment variables (all optional — defaults to deployments.json values):
#   TOKEN_ID     — deployed token contract ID
#   VESTING_ID   — deployed vesting contract ID
#   MULTISIG_ID  — deployed multisig contract ID
#   STELLAR_NETWORK  — "testnet" (default) or "mainnet"
#
# Examples:
#   # Use deployments.json defaults (testnet token + vesting)
#   ./scripts/check-ttl.sh
#
#   # Override individual contract IDs
#   VESTING_ID=CABC... ./scripts/check-ttl.sh
#
#   # Check mainnet contracts
#   STELLAR_NETWORK=mainnet TOKEN_ID=... VESTING_ID=... ./scripts/check-ttl.sh
#
# Prerequisites:
#   stellar-cli installed: curl -sSfL https://install.stellar.org | sh
#   jq installed:          apt install jq  /  brew install jq
#
# Notes:
#   INSTANCE_BUMP_THRESHOLD = 518 400 ledgers (~30 days at 5 s/ledger).
#   This is the value below which the contracts auto-extend their instance TTL
#   on the next state-changing call.  Read-only contracts that receive no
#   state-changing calls will NOT auto-extend, so a TTL drop below this
#   threshold is a genuine warning for contracts that are only ever queried.
#
#   See docs/ttl.md for the full TTL reference.
# =============================================================================

set -euo pipefail

# ── Constants (must match contracts/*/src/lib.rs) ─────────────────────────────

readonly INSTANCE_BUMP_THRESHOLD=518400  # ~30 days

# ── Load .env if present (non-fatal) ─────────────────────────────────────────

if [ -f ".env" ]; then
  set -o allexport && source .env && set +o allexport
fi

# ── Defaults from deployments.json ───────────────────────────────────────────
# Fall back to the recorded testnet deployments when env vars are not set.

DEPLOYMENTS_FILE="$(dirname "$0")/../deployments.json"

_jq_field() {
  local field="$1"
  if [ -f "$DEPLOYMENTS_FILE" ] && command -v jq &>/dev/null; then
    jq -r ".$field // empty" "$DEPLOYMENTS_FILE" 2>/dev/null || true
  fi
}

TOKEN_ID="${TOKEN_ID:-$(_jq_field "token_testnet")}"
VESTING_ID="${VESTING_ID:-$(_jq_field "vesting_testnet")}"
MULTISIG_ID="${MULTISIG_ID:-}"   # no default; multisig not deployed on testnet

NETWORK="${STELLAR_NETWORK:-testnet}"

# ── Helper: get TTL for a contract instance ────────────────────────────────────
# `stellar contract read` returns a JSON array of ledger entries.  Each entry
# has a `liveUntilLedgerSeq` field which is the last ledger at which the entry
# is live.  We subtract the current ledger to get remaining TTL.
#
# If the contract is not deployed or stellar-cli is unavailable the function
# prints a "SKIP" result instead of aborting the whole script.

get_ttl() {
  local contract_id="$1"
  local label="$2"

  if [ -z "$contract_id" ]; then
    echo "  $label : SKIPPED (no contract ID set)"
    return
  fi

  local raw
  if ! raw=$(stellar contract read \
    --id "$contract_id" \
    --network "$NETWORK" \
    --output json 2>&1); then
    echo "  $label ($contract_id) : ERROR — could not read contract"
    echo "    $raw" | head -3
    return
  fi

  # `liveUntilLedgerSeq` is the ledger at which the entry expires.
  # Subtract the current ledger to get remaining ledgers.
  local live_until
  live_until=$(echo "$raw" | jq -r '.[0].liveUntilLedgerSeq // empty' 2>/dev/null || true)

  if [ -z "$live_until" ]; then
    echo "  $label ($contract_id) : UNKNOWN (could not parse liveUntilLedgerSeq)"
    return
  fi

  # Fetch the current ledger sequence.
  local current_ledger
  current_ledger=$(stellar ledger \
    --network "$NETWORK" \
    --output json 2>/dev/null \
    | jq -r '.sequence // empty' 2>/dev/null || echo "0")

  local remaining=$(( live_until - current_ledger ))

  # Approximate days at 5 s/ledger: remaining * 5 / 86400
  local approx_days=$(( remaining * 5 / 86400 ))

  if [ "$remaining" -lt "$INSTANCE_BUMP_THRESHOLD" ]; then
    echo "  ⚠  $label ($contract_id)"
    echo "     TTL: $remaining ledgers (~${approx_days} days) — BELOW THRESHOLD ($INSTANCE_BUMP_THRESHOLD ledgers)"
    echo "     Call extend_ttl to avoid expiry:"
    echo "       stellar contract invoke --id $contract_id --source <ACCOUNT> --network $NETWORK -- extend_ttl"
  else
    echo "  ✓  $label ($contract_id)"
    echo "     TTL: $remaining ledgers (~${approx_days} days) — OK"
  fi
}

# ── Main ──────────────────────────────────────────────────────────────────────

echo "========================================"
echo " Contract TTL Check"
echo " Network  : $NETWORK"
echo " Threshold: $INSTANCE_BUMP_THRESHOLD ledgers (~30 days)"
echo "========================================"
echo ""

get_ttl "$TOKEN_ID"    "Token contract   "
echo ""
get_ttl "$VESTING_ID"  "Vesting contract "
echo ""
get_ttl "$MULTISIG_ID" "Multisig contract"
echo ""

echo "========================================"
echo " Done."
echo "========================================"

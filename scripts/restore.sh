#!/usr/bin/env bash
# =============================================================================
# scripts/restore.sh — Restore archived Soroban contract instance storage
#
# When a contract's instance entry expires (TTL lapses) it is archived and the
# contract cannot be invoked until the instance is restored. This script
# restores the instance storage for the token and vesting contracts using the
# deployed IDs from deployments.json.
#
# Usage:
#   ./scripts/restore.sh
#
# Environment (all optional):
#   SOURCE_KEY   stellar-cli account name used as --source  (default: my-admin)
#   NETWORK      stellar network                            (default: testnet)
#   TOKEN_ID     token contract instance id                 (default: from deployments.json)
#   VESTING_ID   vesting contract instance id               (default: from deployments.json)
#
# Prerequisites:
#   - stellar-cli installed (https://install.stellar.org)
#   - The source account must be funded and authorized as the contracts' admin.
#
# Persistent entries (schedules, balances, allowances, proposals, confirmations)
# are NOT covered by instance restore. Restore an archived persistent entry by
# its storage key instead:
#
#   stellar contract restore \\
#     --id <CONTRACT_ID> \\
#     --source <KEY> \\
#     --network testnet \\
#     --key <XDR_STORAGE_KEY>
#
# The XDR storage key for a persistent entry can be obtained from the contract's
# storage (e.g. DataKey::Schedule(id), DataKey::Balance(address)). See docs/ttl.md
# for the full list of persistent keys per contract.
#
# Persistent entry keys per contract (docs/ttl.md):
#   vesting:  Schedule(id), BeneficiarySchedules(address)
#   token:    Balance(address), Allowance(owner, spender)
#   multisig: Proposal(id), Confirm(proposal_id, address)
# =============================================================================

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
SOURCE_KEY="${SOURCE_KEY:-my-admin}"

# Deployed contract instance IDs (see deployments.json).
DEPLOYMENTS_FILE="${DEPLOYMENTS_FILE:-$(dirname "$0")/../deployments.json}"
if [[ -f "$DEPLOYMENTS_FILE" ]]; then
    TOKEN_ID="${TOKEN_ID:-$(grep -o '"token_testnet": *"[^"]*"' "$DEPLOYMENTS_FILE" | grep -o '"[^"]*"$' | tr -d '"')}"
    VESTING_ID="${VESTING_ID:-$(grep -o '"vesting_testnet": *"[^"]*"' "$DEPLOYMENTS_FILE" | grep -o '"[^"]*"$' | tr -d '"')}"
fi
TOKEN_ID="${TOKEN_ID:-CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH}"
VESTING_ID="${VESTING_ID:-CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF}"

echo "=================================================="
echo " Soroban Vesting Platform — Restore Archived Contracts"
echo "=================================================="
echo " Network : $NETWORK"
echo " Source  : $SOURCE_KEY"
echo " Token   : $TOKEN_ID"
echo " Vesting : $VESTING_ID"
echo

restore_instance() {
    local id="$1"
    local name="$2"
    echo "Restoring instance storage for $name ($id)..."
    stellar contract restore --id "$id" --source "$SOURCE_KEY" --network "$NETWORK"
    echo "     $name instance restored."
    echo
}

restore_instance "$TOKEN_ID" "token contract"
restore_instance "$VESTING_ID" "vesting contract"

echo "=================================================="
echo " Instance restore complete."
echo "=================================================="
echo
echo "Note: This restores the contract instance entries only. Persistent entries"
echo "(vesting schedules/beneficiary lists, token balances/allowances, multisig"
echo "proposals/confirmations) are not covered by instance restore. To restore an"
echo "archived persistent entry by storage key, run:"
echo
echo "  stellar contract restore \\"
echo "    --id <CONTRACT_ID> \\"
echo "    --source $SOURCE_KEY \\"
echo "    --network $NETWORK \\"
echo "    --key <XDR_STORAGE_KEY>"

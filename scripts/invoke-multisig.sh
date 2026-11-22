#!/usr/bin/env bash
# =============================================================================
# invoke-multisig.sh — Helper for multisig operations (submit / confirm / execute)
#
# Wraps `stellar contract invoke` for the deployed multisig contract so
# operators do not have to memorise the argument syntax of each function.
#
# Usage:
#   ./scripts/invoke-multisig.sh submit <PROPOSER_KEY> <ACTION_JSON> <DESCRIPTION>
#   ./scripts/invoke-multisig.sh confirm <OWNER_KEY> <PROPOSAL_ID>
#   ./scripts/invoke-multisig.sh execute <PROPOSAL_ID>
#
# Environment (set in .env or exported):
#   MULTISIG_CONTRACT_ID  (required) — address of the deployed multisig contract
#   SOURCE_ACCOUNT        (fallback signer for `execute`) — secret key / keypair name
#   NETWORK               (optional, default testnet)
#
# Examples:
#   export MULTISIG_CONTRACT_ID=CABC…XYZ
#   ./scripts/invoke-multisig.sh submit GABC…PROPOSER \
#       '{"Call":{"target":"$VESTING_ID","function":"create_schedule","args":[...]}}' \
#       'Create vesting schedule'
#   ./scripts/invoke-multisig.sh confirm GABC…OWNER 1
#   ./scripts/invoke-multisig.sh execute 1
#
# Notes:
#   - `submit` and `confirm` require the proposer/owner to sign, so the key
#     argument is used as `--source`. Store keys with `stellar keys store` or
#     pass a public address (G…) directly.
#   - `execute` is permissionless once threshold is met; `SOURCE_ACCOUNT` signs it.
# =============================================================================

set -euo pipefail

if [ -f ".env" ]; then
  set -o allexport && source .env && set +o allexport
fi

MULTISIG_ID="${MULTISIG_CONTRACT_ID:?Set MULTISIG_CONTRACT_ID in .env or export it}"
NETWORK="${NETWORK:-testnet}"
RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
if [ "$NETWORK" = "mainnet" ]; then
  RPC_URL="https://soroban-mainnet.stellar.org"
  NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
fi

# Resolve a key argument to a public address. The key may be a stored
# stellar-keypair name or a public address (G…). Raw secret keys are accepted
# only as --source signers; the address argument still needs a public address.
resolve_address() {
  local key="$1"
  local address=""
  if stellar keys address "$key" >/dev/null 2>&1; then
    address="$(stellar keys address "$key")"
  elif [[ "$key" == G* ]]; then
    address="$key"
  else
    echo "Error: '$key' is not a stored keypair name or a public address (G…)." >&2
    echo "       Store it first: stellar keys store <name> --secret-key <secret>" >&2
    exit 1
  fi
  echo "$address"
}

invoke() {
  local source="$1"; shift
  stellar contract invoke \
    --id "$MULTISIG_ID" \
    --source "$source" \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- "$@"
}

cmd="${1:?Usage: invoke-multisig.sh <submit|confirm|execute> <args...>}"
shift || true

case "$cmd" in
  submit)
    # ./scripts/invoke-multisig.sh submit <PROPOSER_KEY> <ACTION_JSON> <DESCRIPTION>
    #
    # submit(proposer: Address, action: ProposalAction, description: String) -> u64
    # The proposer must authorise the call, so they sign the transaction.
    proposer_key="${1:?Missing PROPOSER_KEY}"
    action_json="${2:?Missing ACTION_JSON}"
    description="${3:?Missing DESCRIPTION}"
    proposer_address="$(resolve_address "$proposer_key")"
    invoke "$proposer_key" submit "$proposer_address" "$action_json" "$description"
    ;;
  confirm)
    # ./scripts/invoke-multisig.sh confirm <OWNER_KEY> <PROPOSAL_ID>
    #
    # confirm(owner: Address, proposal_id: u64)
    # The confirming owner must authorise the call, so they sign the transaction.
    owner_key="${1:?Missing OWNER_KEY}"
    proposal_id="${2:?Missing PROPOSAL_ID}"
    owner_address="$(resolve_address "$owner_key")"
    invoke "$owner_key" confirm "$owner_address" "$proposal_id"
    ;;
  execute)
    # ./scripts/invoke-multisig.sh execute <PROPOSAL_ID>
    #
    # execute(proposal_id: u64) — permissionless once threshold is met.
    proposal_id="${1:?Missing PROPOSAL_ID}"
    source="${SOURCE_ACCOUNT:?Set SOURCE_ACCOUNT in .env or export it}"
    invoke "$source" execute "$proposal_id"
    ;;
  *)
    echo "Unknown command: $cmd" >&2
    echo "Usage: $0 <submit|confirm|execute> <args...>" >&2
    exit 1
    ;;
esac

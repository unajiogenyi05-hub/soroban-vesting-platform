# Off-chain Keep-alive: Automating extend_ttl()

Soroban instance storage expires after ~180 days of inactivity (see
[docs/ttl.md](ttl.md) for the exact constants). When instance storage expires
the entire contract becomes unresponsive until an operator manually restores it
(see [docs/disaster-recovery.md](disaster-recovery.md)).

This guide explains why a keep-alive bot is necessary, provides working shell
and Node.js examples, and shows how to schedule them as a cron job or GitHub
Actions workflow.

---

## Why keep-alive is needed

All three contracts use a threshold/bump pattern:

| Constant | Value | Approx. |
|----------|-------|---------|
| `INSTANCE_BUMP_THRESHOLD` | 518 400 ledgers | ~30 days |
| `INSTANCE_BUMP_LEDGERS` | 3 110 400 ledgers | ~180 days |

The instance TTL is bumped on every mutating call (`mint`, `create_schedule`,
`submit`, etc.). For actively used contracts this is automatic. For idle
deployments — contracts that see no on-chain calls for 30+ days — the TTL will
fall below the threshold and will not be extended until the next call. If it
reaches zero the instance is archived.

Each contract exposes a permissionless `extend_ttl()` entry point. Any funded
account can call it without admin keys to bump the instance TTL back to the
maximum (~180 days). Calling it once every ~150 days is enough to keep all
three contracts live indefinitely.

---

## Shell script example

This script calls `extend_ttl()` on all three contracts. Set the four
environment variables before running.

```bash
#!/usr/bin/env bash
# scripts/keepalive.sh
# Calls extend_ttl() on all three Soroban contracts to prevent instance archival.
# Run once every ~150 days (or on a cron schedule — see below).
#
# Required environment variables:
#   KEEPALIVE_ACCOUNT  — stellar-cli key name (e.g. "my-admin")
#   NETWORK            — "testnet" or "mainnet"
#   TOKEN_ID           — Token contract ID
#   VESTING_ID         — Vesting contract ID
#   MULTISIG_ID        — Multisig contract ID (optional; skip if not deployed)

set -euo pipefail

: "${KEEPALIVE_ACCOUNT:?Set KEEPALIVE_ACCOUNT to a funded stellar-cli key name}"
: "${NETWORK:?Set NETWORK to testnet or mainnet}"
: "${TOKEN_ID:?Set TOKEN_ID to the token contract address}"
: "${VESTING_ID:?Set VESTING_ID to the vesting contract address}"

extend_ttl() {
  local name="$1"
  local id="$2"
  echo "→ Extending TTL for $name ($id)..."
  stellar contract invoke \
    --id "$id" \
    --source "$KEEPALIVE_ACCOUNT" \
    --network "$NETWORK" \
    -- extend_ttl
  echo "  ✓ $name TTL extended"
}

extend_ttl "Token"   "$TOKEN_ID"
extend_ttl "Vesting" "$VESTING_ID"

# Multisig is optional — only call if the variable is set
if [[ -n "${MULTISIG_ID:-}" ]]; then
  extend_ttl "Multisig" "$MULTISIG_ID"
fi

echo "Keep-alive complete."
```

Make it executable and run a quick test on testnet:

```bash
chmod +x scripts/keepalive.sh

export KEEPALIVE_ACCOUNT=my-admin
export NETWORK=testnet
export TOKEN_ID=CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH
export VESTING_ID=CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF
# export MULTISIG_ID=<your multisig contract ID if deployed>

./scripts/keepalive.sh
```

---

## Node.js example

This script uses `@stellar/stellar-sdk` to simulate and submit `extend_ttl()`
for each contract. It does not require a local stellar-cli key store; pass the
secret key via an environment variable.

```javascript
// scripts/keepalive.js
// npm install @stellar/stellar-sdk
import {
  SorobanRpc,
  TransactionBuilder,
  Networks,
  Keypair,
  BASE_FEE,
  xdr,
  Address,
  nativeToScVal,
} from "@stellar/stellar-sdk";

const RPC_URL = process.env.RPC_URL || "https://soroban-testnet.stellar.org";
const SECRET_KEY = process.env.SECRET_KEY; // never hardcode this
const NETWORK_PASSPHRASE = process.env.NETWORK === "mainnet"
  ? Networks.PUBLIC
  : Networks.TESTNET;

const CONTRACT_IDS = [
  { name: "Token",   id: process.env.TOKEN_ID },
  { name: "Vesting", id: process.env.VESTING_ID },
  { name: "Multisig", id: process.env.MULTISIG_ID },
].filter((c) => c.id); // skip undefined entries

async function extendTtl(server, keypair, contractId, name) {
  const account = await server.getAccount(keypair.publicKey());
  const contract = new Address(contractId).toScAddress();

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      xdr.Operation.invokeHostFunction({
        hostFunction: xdr.HostFunction.hostFunctionTypeInvokeContract(
          new xdr.InvokeContractArgs({
            contractAddress: contract,
            functionName: "extend_ttl",
            args: [],
          })
        ),
        auth: [],
      })
    )
    .setTimeout(30)
    .build();

  const simResult = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(simResult)) {
    throw new Error(`Simulation failed for ${name}: ${simResult.error}`);
  }

  const preparedTx = SorobanRpc.assembleTransaction(tx, simResult).build();
  preparedTx.sign(keypair);

  const sendResult = await server.sendTransaction(preparedTx);
  if (sendResult.status === "ERROR") {
    throw new Error(`Submit failed for ${name}: ${sendResult.errorResult}`);
  }

  // Poll until confirmed
  let getResult;
  do {
    await new Promise((r) => setTimeout(r, 2000));
    getResult = await server.getTransaction(sendResult.hash);
  } while (getResult.status === SorobanRpc.Api.GetTransactionStatus.NOT_FOUND);

  if (getResult.status !== SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`Transaction failed for ${name}: ${getResult.status}`);
  }

  console.log(`✓ ${name} TTL extended (tx: ${sendResult.hash})`);
}

async function main() {
  if (!SECRET_KEY) throw new Error("Set SECRET_KEY environment variable");

  const server = new SorobanRpc.Server(RPC_URL);
  const keypair = Keypair.fromSecret(SECRET_KEY);

  for (const { name, id } of CONTRACT_IDS) {
    await extendTtl(server, keypair, id, name);
  }

  console.log("Keep-alive complete.");
}

main().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
```

Run it:

```bash
export SECRET_KEY=S...             # a funded Stellar secret key — never commit it
export TOKEN_ID=CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH
export VESTING_ID=CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF
# export MULTISIG_ID=<your multisig contract ID>

node scripts/keepalive.js
```

---

## Scheduling as a cron job

Add this to crontab (`crontab -e`) to run every 150 days:

```cron
# Run contract keep-alive every 150 days at 03:00 UTC
0 3 */150 * * /path/to/scripts/keepalive.sh >> /var/log/keepalive.log 2>&1
```

A simpler approach on most Linux systems — run monthly (well inside the
30-day threshold window):

```cron
# Run on the 1st of every month at 03:00 UTC
0 3 1 * * /path/to/scripts/keepalive.sh >> /var/log/keepalive.log 2>&1
```

Make sure the cron environment has `KEEPALIVE_ACCOUNT`, `NETWORK`,
`TOKEN_ID`, and `VESTING_ID` set. Load them from a file:

```bash
# At the top of keepalive.sh, before the : checks:
source /etc/keepalive.env   # contains the env vars, chmod 600
```

---

## Scheduling as a GitHub Actions workflow

Create `.github/workflows/keepalive.yml`:

```yaml
name: Contract Keep-alive

on:
  schedule:
    # Runs on the 1st of every month at 03:00 UTC
    - cron: "0 3 1 * *"
  workflow_dispatch: # allow manual trigger

jobs:
  keepalive:
    name: Extend contract TTLs
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install stellar-cli
        uses: stellar/stellar-cli@v28.0.0

      - name: Import signing key
        run: |
          stellar keys add keepalive-bot --secret-key
        env:
          STELLAR_SECRET_KEY: ${{ secrets.KEEPALIVE_SECRET_KEY }}

      - name: Extend Token TTL
        run: |
          stellar contract invoke \
            --id "$TOKEN_ID" \
            --source keepalive-bot \
            --network testnet \
            -- extend_ttl
        env:
          TOKEN_ID: ${{ vars.TOKEN_CONTRACT_ID }}

      - name: Extend Vesting TTL
        run: |
          stellar contract invoke \
            --id "$VESTING_ID" \
            --source keepalive-bot \
            --network testnet \
            -- extend_ttl
        env:
          VESTING_ID: ${{ vars.VESTING_CONTRACT_ID }}

      - name: Extend Multisig TTL
        if: vars.MULTISIG_CONTRACT_ID != ''
        run: |
          stellar contract invoke \
            --id "$MULTISIG_ID" \
            --source keepalive-bot \
            --network testnet \
            -- extend_ttl
        env:
          MULTISIG_ID: ${{ vars.MULTISIG_CONTRACT_ID }}
```

**Repository secrets and variables to configure:**

| Name | Type | Value |
|------|------|-------|
| `KEEPALIVE_SECRET_KEY` | Secret | A funded Stellar secret key (`S...`). Do not use the deployment admin key — use a dedicated key with only enough XLM for fees. |
| `TOKEN_CONTRACT_ID` | Variable | Token contract address |
| `VESTING_CONTRACT_ID` | Variable | Vesting contract address |
| `MULTISIG_CONTRACT_ID` | Variable | Multisig contract address (leave empty if not deployed) |

---

## Security notes

- Keep-alive calls require only XLM for transaction fees; they do not require
  admin privileges. Use a dedicated, minimally funded account rather than the
  deployment admin key.
- Store the secret key in GitHub Secrets or a vault — never in source code,
  config files, or environment files committed to the repository.
- If the keep-alive account runs out of XLM, `extend_ttl()` calls will fail
  silently. Monitor the account balance alongside the workflow run status.

---

## Reference

- [docs/ttl.md](ttl.md) — TTL constants and what expires
- [docs/disaster-recovery.md](disaster-recovery.md) — restoring an already-archived contract
- [`deployments.json`](../deployments.json) — testnet contract IDs

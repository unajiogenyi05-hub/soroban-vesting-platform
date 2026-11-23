#!/usr/bin/env node
/**
 * query-claimable.js
 *
 * Example: query get_claimable(schedule_id) on the vesting contract directly
 * from Node.js using @stellar/stellar-sdk, without going through the backend
 * REST API.  This shows the simulation pattern that developers can adapt when
 * building their own integrations.
 *
 * Usage:
 *   node scripts/examples/query-claimable.js
 *
 * Environment variables:
 *   VESTING_CONTRACT_ID  — deployed vesting contract address (required)
 *   SCHEDULE_ID          — schedule ID to query (default: 1)
 *   SOURCE_PUBLIC_KEY    — Stellar public key used as the fee source for the
 *                          simulation; must exist on-chain (default: a funded
 *                          testnet account from deployments.json docs)
 *   STELLAR_RPC_URL      — Soroban RPC endpoint (default: testnet)
 *   STELLAR_NETWORK      — "testnet" or "mainnet" (default: "testnet")
 *
 * Prerequisites:
 *   cd backend && npm ci   (installs @stellar/stellar-sdk)
 *
 * Run from the repository root:
 *   VESTING_CONTRACT_ID=CDBSJE... node scripts/examples/query-claimable.js
 *
 * How it works:
 *   get_claimable is a pure read-only function.  Rather than submitting a
 *   signed transaction, we build an unsigned transaction and pass it to
 *   server.simulateTransaction().  The RPC node executes the WASM in a
 *   sandbox and returns the return value without touching the ledger.
 */

'use strict';

// ── Resolve the stellar-sdk from the backend's node_modules ──────────────────
// The package is declared as a dependency in backend/package.json.
// From the repo root, require it with a relative path.
const {
  SorobanRpc,
  Contract,
  Keypair,
  Networks,
  TransactionBuilder,
  BASE_FEE,
  nativeToScVal,
  scValToNative,
} = require('../backend/node_modules/@stellar/stellar-sdk');

// ── Configuration (read from environment with safe defaults) ──────────────────

const VESTING_CONTRACT_ID = process.env.VESTING_CONTRACT_ID
  || 'CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF';

const SCHEDULE_ID = parseInt(process.env.SCHEDULE_ID || '1', 10);

// A simulation still needs an on-chain source account to build the transaction
// envelope.  Any funded account works — it is not required to sign.
// Default to a well-known testnet account; override via SOURCE_PUBLIC_KEY.
const SOURCE_PUBLIC_KEY = process.env.SOURCE_PUBLIC_KEY
  || 'GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN';

const RPC_URL = process.env.STELLAR_RPC_URL
  || 'https://soroban-testnet.stellar.org';

const NETWORK = process.env.STELLAR_NETWORK || 'testnet';
const NETWORK_PASSPHRASE = NETWORK === 'mainnet'
  ? Networks.PUBLIC
  : Networks.TESTNET;

// ── Main ──────────────────────────────────────────────────────────────────────

async function main() {
  console.log('=== query-claimable.js ===');
  console.log(`Network        : ${NETWORK}`);
  console.log(`RPC URL        : ${RPC_URL}`);
  console.log(`Contract       : ${VESTING_CONTRACT_ID}`);
  console.log(`Schedule ID    : ${SCHEDULE_ID}`);
  console.log(`Source account : ${SOURCE_PUBLIC_KEY}`);
  console.log('');

  // 1. Connect to the Soroban RPC node.
  const server = new SorobanRpc.Server(RPC_URL, {
    allowHttp: RPC_URL.startsWith('http://'),
  });

  // 2. Load the source account so we can build a valid transaction envelope.
  //    The simulation does not require a signature, but TransactionBuilder
  //    needs a loaded AccountResponse to set the sequence number.
  let account;
  try {
    account = await server.getAccount(SOURCE_PUBLIC_KEY);
  } catch (err) {
    console.error(`Failed to load account ${SOURCE_PUBLIC_KEY}:`);
    console.error(err.message);
    console.error('');
    console.error('Make sure SOURCE_PUBLIC_KEY is a funded testnet account.');
    console.error('Fund a new account: stellar keys generate my-key --network testnet --fund');
    process.exit(1);
  }

  // 3. Build the contract invocation operation.
  //    get_claimable(schedule_id: u64) → i128
  const contract = new Contract(VESTING_CONTRACT_ID);

  const scheduleIdScVal = nativeToScVal(SCHEDULE_ID, { type: 'u64' });

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call('get_claimable', scheduleIdScVal))
    .setTimeout(30)
    .build();

  // 4. Simulate the transaction (read-only — no signing needed).
  console.log('Simulating get_claimable...');
  let simResult;
  try {
    simResult = await server.simulateTransaction(tx);
  } catch (err) {
    console.error('RPC error during simulation:');
    console.error(err.message);
    process.exit(1);
  }

  if (SorobanRpc.Api.isSimulationError(simResult)) {
    console.error('Contract simulation returned an error:');
    console.error(simResult.error);
    console.error('');
    console.error('Common causes:');
    console.error('  - Schedule ID does not exist (ScheduleNotFound)');
    console.error('  - Contract ID is wrong or not deployed');
    process.exit(1);
  }

  // 5. Decode the return value from XDR ScVal to a native JS value.
  //    get_claimable returns i128 — scValToNative gives us a BigInt.
  const returnVal = simResult.result?.retval;
  if (!returnVal) {
    console.error('Simulation succeeded but returned no value.');
    process.exit(1);
  }

  const claimable = scValToNative(returnVal);

  // 6. Print the result.
  console.log('');
  console.log(`Claimable amount for schedule ${SCHEDULE_ID}: ${claimable}`);
  console.log('');
  console.log('Raw simulation result (cost breakdown):');
  console.log(`  CPU instructions : ${simResult.cost?.cpuInsns ?? 'n/a'}`);
  console.log(`  Memory bytes     : ${simResult.cost?.memBytes ?? 'n/a'}`);
  console.log('');
  console.log('Done.');
}

main().catch((err) => {
  console.error('Unexpected error:', err.message);
  process.exit(1);
});

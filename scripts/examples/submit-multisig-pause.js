#!/usr/bin/env node
/**
 * submit-multisig-pause.js
 *
 * Builds and simulates a multisig proposal that pauses the vesting contract,
 * demonstrating the full SDK-level flow for the most common governance
 * operation:
 *   ProposalAction::Call { target: vesting, function: "pause", args: [] }
 *
 * Usage:
 *   node scripts/examples/submit-multisig-pause.js \\
 *     <MULTISIG_CONTRACT_ID> <VESTING_CONTRACT_ID> <PROPOSER_SECRET_KEY> \\
 *     [--network testnet|mainnet]
 *
 * Steps:
 *   1. Build the CallData ScVal for vesting.pause() using nativeToScVal
 *   2. Wrap it as ProposalAction::Call (enum discriminant 0 + CallData struct)
 *   3. Build submit() args: [proposer, action, description]
 *   4. Simulate multisig.submit() via Soroban RPC
 *   5. Print the simulated transaction XDR for inspection
 */

const {
  SorobanRpc,
  Keypair,
  Networks,
  TransactionBuilder,
  BASE_FEE,
  nativeToScVal,
  ScVal,
  Contract,
} = require('@stellar/stellar-sdk');

const argv = process.argv.slice(2);
const networkFlag = argv.find((a) => a.startsWith('--network=')) || argv.find((a) => a === '--network');
const isMainnet =
  (networkFlag && networkFlag.includes('mainnet')) ||
  (argv[argv.indexOf('--network') + 1] === 'mainnet');

const NETWORK = isMainnet ? 'mainnet' : 'testnet';
const NETWORK_PASSPHRASE = isMainnet ? Networks.PUBLIC : Networks.TESTNET;
const RPC_URL = isMainnet
  ? 'https://soroban-mainnet.stellar.org'
  : 'https://soroban-testnet.stellar.org';

const positional = argv.filter((a) => !a.startsWith('--'));
const [MULTISIG_ID, VESTING_ID, SECRET_KEY] = positional;

if (!MULTISIG_ID || !VESTING_ID || !SECRET_KEY) {
  console.error(
    'Usage: node submit-multisig-pause.js <MULTISIG_CONTRACT_ID> <VESTING_CONTRACT_ID> <PROPOSER_SECRET_KEY> [--network testnet|mainnet]'
  );
  process.exit(1);
}

const server = new SorobanRpc.Server(RPC_URL, { allowHttp: false });
const proposer = Keypair.fromSecret(SECRET_KEY);

async function main() {
  // ── 1. Build the CallData ScVal for vesting.pause() ──────────────────
  // CallData { target: Address, function: Symbol, args: Vec<Val> }
  const targetScVal = nativeToScVal(VESTING_ID, { type: 'address' });
  const functionScVal = nativeToScVal('pause', { type: 'symbol' });
  const argsScVal = nativeToScVal([], {
    type: 'vec',
    elementType: { type: 'u32' },
  });

  // CallData is a contracttype struct → serialized as a vec of its fields.
  const callDataScVal = ScVal.vec([targetScVal, functionScVal, argsScVal]);

  // ── 2. Wrap as ProposalAction::Call (enum discriminant = 0) ──────────
  // ProposalAction::Call(CallData) → vec [u32(0), callData]
  const actionScVal = ScVal.vec([ScVal.u32(0), callDataScVal]);

  // ── 3. Build submit() args ───────────────────────────────────────────
  // submit(proposer: Address, action: ProposalAction, description: String)
  const proposerScVal = nativeToScVal(proposer.publicKey(), { type: 'address' });
  const descriptionScVal = nativeToScVal(
    'Pause vesting via multisig proposal',
    { type: 'string' }
  );

  // ── 4. Load account, build, simulate ─────────────────────────────────
  const account = await server.getAccount(proposer.publicKey());

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      new Contract(MULTISIG_ID).call(
        'submit',
        proposerScVal,
        actionScVal,
        descriptionScVal
      )
    )
    .setTimeout(30)
    .build();

  const simResult = await server.simulateTransaction(tx);

  if (SorobanRpc.Api.isSimulationError(simResult)) {
    console.error('Simulation error:', simResult.error);
    process.exit(1);
  }

  // ── 5. Print simulated transaction XDR ───────────────────────────────
  console.log('=== Simulated multisig.submit() transaction XDR ===');
  console.log(tx.toXDR());
  console.log('');
  console.log('=== Simulation result ===');
  console.log(JSON.stringify(simResult, null, 2));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

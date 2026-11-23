'use strict';

/**
 * StellarService
 *
 * Thin wrapper around @stellar/stellar-sdk for invoking Soroban contracts.
 * All functions return { success, data } or throw with a descriptive message.
 */

const { SorobanRpc, Networks, TransactionBuilder, BASE_FEE, StrKey } = require('@stellar/stellar-sdk');

const NETWORK_PASSPHRASE = process.env.STELLAR_NETWORK === 'mainnet'
  ? Networks.PUBLIC
  : Networks.TESTNET;

const RPC_URL = process.env.STELLAR_RPC_URL || 'https://soroban-testnet.stellar.org';

/**
 * Validate a Stellar Ed25519 public key (56-character G… string).
 * @param {string} addr
 * @returns {boolean}
 */
function validateStellarAddress(addr) {
  return typeof addr === 'string' && StrKey.isValidEd25519PublicKey(addr);
}

/**
 * Build a Soroban RPC server instance.
 */
function getServer() {
  return new SorobanRpc.Server(RPC_URL, { allowHttp: RPC_URL.startsWith('http://') });
}

/**
 * Load an account from the network.
 * @param {string} publicKey
 */
async function loadAccount(publicKey) {
  const server = getServer();
  return server.getAccount(publicKey);
}

/**
 * Submit a pre-built signed transaction and wait for confirmation.
 * @param {Transaction} tx
 */
async function submitTransaction(tx) {
  const server = getServer();
  let response = await server.sendTransaction(tx);

  if (response.status === 'ERROR') {
    throw new Error(`Transaction failed: ${JSON.stringify(response.errorResult)}`);
  }

  // Poll until confirmed
  const hash = response.hash;
  for (let i = 0; i < 30; i++) {
    await new Promise((r) => setTimeout(r, 2000));
    const status = await server.getTransaction(hash);
    if (status.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
      return status;
    }
    if (status.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
      throw new Error(`Transaction failed on-chain: ${hash}`);
    }
  }
  throw new Error(`Transaction timed out: ${hash}`);
}

/**
 * Build and simulate a Soroban contract call (read-only or preflight).
 */
async function simulateContractCall({ contractId, method, args, sourceKeypair }) {
  const server = getServer();
  const account = await server.getAccount(sourceKeypair.publicKey());
  const contract = new (require('@stellar/stellar-sdk').Contract)(contractId);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  const simResult = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(simResult)) {
    throw new Error(`Simulation error: ${simResult.error}`);
  }
  return simResult;
}

module.exports = {
  getServer,
  loadAccount,
  submitTransaction,
  simulateContractCall,
  validateStellarAddress,
  NETWORK_PASSPHRASE,
  RPC_URL,
};

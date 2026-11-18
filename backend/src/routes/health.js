'use strict';

const { Router } = require('express');
const { RPC_URL, NETWORK_PASSPHRASE } = require('../services/stellar');

const router = Router();

router.get('/', (req, res) => {
  res.json({
    status: 'ok',
    timestamp: new Date().toISOString(),
    network: process.env.STELLAR_NETWORK || 'testnet',
    rpc: RPC_URL,
  });
});

router.get('/ready', (req, res) => {
  const contracts = {
    vesting: Boolean(process.env.VESTING_CONTRACT_ID),
    token: Boolean(process.env.TOKEN_CONTRACT_ID),
    multisig: Boolean(process.env.MULTISIG_CONTRACT_ID),
  };
  const ready = Object.values(contracts).some(Boolean);
  res.status(ready ? 200 : 503).json({ ready, contracts });
});

module.exports = router;

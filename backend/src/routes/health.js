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

module.exports = router;

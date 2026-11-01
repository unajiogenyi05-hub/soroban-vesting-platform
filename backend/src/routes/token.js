'use strict';

/**
 * Token API Routes
 *
 * GET  /api/token/info              — name, symbol, decimals, total supply
 * GET  /api/token/balance/:addr      — token balance for address
 * GET  /api/token/allowance         — allowance(owner, spender)
 * POST /api/token/mint              — mint tokens (admin)
 * POST /api/token/burn              — burn tokens
 * POST /api/token/transfer          — transfer tokens
 * POST /api/token/approve           — approve spender
 * POST /api/token/pause             — pause token (admin)
 * POST /api/token/unpause           — unpause token (admin)
 */

const { Router } = require('express');
const { Keypair, nativeToScVal, scValToNative } = require('@stellar/stellar-sdk');
const { simulateContractCall, validateStellarAddress } = require('../services/stellar');
const logger = require('../logger');

const router = Router();
const CONTRACT_ID = process.env.TOKEN_CONTRACT_ID || '';

function contractIdMiddleware(req, res, next) {
  const id = req.body?.contractId || req.query?.contractId || CONTRACT_ID;
  if (!id) return res.status(400).json({ error: 'contractId is required' });
  req.contractId = id;
  next();
}

function rejectInvalidAddress(res, field, value) {
  if (!validateStellarAddress(value)) {
    res.status(400).json({ error: `invalid Stellar address: ${field}` });
    return true;
  }
  return false;
}

router.get('/info', contractIdMiddleware, async (req, res, next) => {
  try {
    res.json({
      success: true,
      data: {
        contractId: req.contractId,
        name: 'Vesting Token',
        symbol: 'VEST',
        decimals: 7,
        totalSupply: 0,
        note: 'Set TOKEN_CONTRACT_ID to read live data.',
      },
    });
  } catch (err) {
    next(err);
  }
});

/**
 * GET /api/token/balance/:addr
 *
 * When TOKEN_CONTRACT_ID and SOURCE_SECRET_KEY are configured, simulates the
 * `balance(account)` call on the live contract and returns the real on-chain
 * balance.  Falls back to a stub (balance: 0) when either env var is absent.
 */
router.get('/balance/:addr', contractIdMiddleware, async (req, res, next) => {
  try {
    const { addr } = req.params;
    const sourceSecret = process.env.SOURCE_SECRET_KEY || '';

    if (req.contractId && sourceSecret) {
      // Validate that the address looks like a Stellar account ID (G…).
      if (!validateStellarAddress(addr)) {
        return res.status(400).json({ error: 'invalid Stellar address: addr' });
      }

      const sourceKeypair = Keypair.fromSecret(sourceSecret);
      const addrArg = nativeToScVal(addr, { type: 'address' });

      const simResult = await simulateContractCall({
        contractId: req.contractId,
        method: 'balance',
        args: [addrArg],
        sourceKeypair,
      });

      // simResult.result.retval is an xdr.ScVal encoding an i128.
      const balance = scValToNative(simResult.result.retval);

      logger.info(`balance addr=${addr} balance=${balance}`);
      return res.json({
        success: true,
        data: { address: addr, balance: balance.toString(), contractId: req.contractId },
      });
    }

    // Stub fallback: contract not configured.
    res.json({
      success: true,
      data: {
        address: addr,
        balance: 0,
        contractId: req.contractId,
        note: 'Set TOKEN_CONTRACT_ID and SOURCE_SECRET_KEY to read live balance.',
      },
    });
  } catch (err) {
    next(err);
  }
});

router.get('/allowance', contractIdMiddleware, async (req, res, next) => {
  try {
    const { owner, spender } = req.query;
    if (!owner || !spender) {
      return res.status(400).json({ error: 'owner and spender query params required' });
    }
    res.json({
      success: true,
      data: { owner, spender, allowance: 0, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/mint', contractIdMiddleware, async (req, res, next) => {
  try {
    const { to, amount } = req.body;
    if (!to || !amount) return res.status(400).json({ error: 'to and amount required' });
    if (rejectInvalidAddress(res, 'to', to)) return;
    if (Number(amount) <= 0) return res.status(400).json({ error: 'amount must be positive' });
    logger.info(`mint to=${to} amount=${amount}`);
    res.status(202).json({
      success: true,
      message: 'Mint transaction ready.',
      data: { to, amount, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/burn', contractIdMiddleware, async (req, res, next) => {
  try {
    const { from, amount } = req.body;
    if (!from || !amount) return res.status(400).json({ error: 'from and amount required' });
    if (rejectInvalidAddress(res, 'from', from)) return;
    res.status(202).json({
      success: true,
      message: 'Burn transaction ready.',
      data: { from, amount, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/transfer', contractIdMiddleware, async (req, res, next) => {
  try {
    const { from, to, amount } = req.body;
    if (!from || !to || !amount) {
      return res.status(400).json({ error: 'from, to and amount required' });
    }
    if (rejectInvalidAddress(res, 'from', from)) return;
    if (rejectInvalidAddress(res, 'to', to)) return;
    res.status(202).json({
      success: true,
      message: 'Transfer transaction ready.',
      data: { from, to, amount, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/approve', contractIdMiddleware, async (req, res, next) => {
  try {
    const { owner, spender, amount } = req.body;
    if (!owner || !spender || amount === undefined) {
      return res.status(400).json({ error: 'owner, spender and amount required' });
    }
    if (rejectInvalidAddress(res, 'owner', owner)) return;
    if (rejectInvalidAddress(res, 'spender', spender)) return;
    res.status(202).json({
      success: true,
      message: 'Approve transaction ready.',
      data: { owner, spender, amount, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/pause', contractIdMiddleware, async (req, res, next) => {
  try {
    res.status(202).json({ success: true, message: 'Pause transaction ready.' });
  } catch (err) {
    next(err);
  }
});

router.post('/unpause', contractIdMiddleware, async (req, res, next) => {
  try {
    res.status(202).json({ success: true, message: 'Unpause transaction ready.' });
  } catch (err) {
    next(err);
  }
});

module.exports = router;

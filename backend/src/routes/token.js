'use strict';

/**
 * Token API Routes
 *
 * GET  /api/token/info              — name, symbol, decimals, total supply
 * GET  /api/token/balance/:addr     — token balance for address
 * GET  /api/token/allowance         — allowance(owner, spender)
 * POST /api/token/mint              — mint tokens (admin)
 * POST /api/token/burn              — burn tokens
 * POST /api/token/transfer          — transfer tokens
 * POST /api/token/approve           — approve spender
 * POST /api/token/pause             — pause token (admin)
 * POST /api/token/unpause           — unpause token (admin)
 */

const { Router } = require('express');
const logger = require('../logger');

const router = Router();
const CONTRACT_ID = process.env.TOKEN_CONTRACT_ID || '';

function contractIdMiddleware(req, res, next) {
  const id = req.body?.contractId || req.query?.contractId || CONTRACT_ID;
  if (!id) return res.status(400).json({ error: 'contractId is required' });
  req.contractId = id;
  next();
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

router.get('/balance/:addr', contractIdMiddleware, async (req, res, next) => {
  try {
    res.json({
      success: true,
      data: { address: req.params.addr, balance: 0, contractId: req.contractId },
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

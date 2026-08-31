'use strict';

/**
 * Multisig API Routes
 *
 * POST /api/multisig/proposal          — submit a proposal
 * GET  /api/multisig/proposal/:id      — get proposal details
 * POST /api/multisig/confirm           — confirm a proposal
 * POST /api/multisig/revoke-confirm    — revoke a confirmation
 * POST /api/multisig/execute           — execute a ready proposal
 * POST /api/multisig/cancel            — cancel a proposal
 * GET  /api/multisig/owners            — list current owners
 * GET  /api/multisig/threshold         — get current threshold
 * GET  /api/multisig/count             — proposal count
 */

const { Router } = require('express');
const logger = require('../logger');

const router = Router();
const CONTRACT_ID = process.env.MULTISIG_CONTRACT_ID || '';

function contractIdMiddleware(req, res, next) {
  const id = req.body?.contractId || req.query?.contractId || CONTRACT_ID;
  if (!id) return res.status(400).json({ error: 'contractId is required' });
  req.contractId = id;
  next();
}

router.post('/proposal', contractIdMiddleware, async (req, res, next) => {
  try {
    const { proposer, description } = req.body;
    if (!proposer || !description) {
      return res.status(400).json({ error: 'proposer and description are required' });
    }
    logger.info(`submit proposal proposer=${proposer}`);
    res.status(202).json({
      success: true,
      message: 'Proposal transaction ready for signing.',
      data: { proposer, description, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.get('/proposal/:id', contractIdMiddleware, async (req, res, next) => {
  try {
    res.json({
      success: true,
      data: {
        id: Number(req.params.id),
        contractId: req.contractId,
        note: 'Set MULTISIG_CONTRACT_ID to read live data.',
      },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/confirm', contractIdMiddleware, async (req, res, next) => {
  try {
    const { owner, proposalId } = req.body;
    if (!owner || !proposalId) {
      return res.status(400).json({ error: 'owner and proposalId are required' });
    }
    logger.info(`confirm proposal=${proposalId} owner=${owner}`);
    res.status(202).json({
      success: true,
      message: 'Confirm transaction ready.',
      data: { owner, proposalId, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/revoke-confirm', contractIdMiddleware, async (req, res, next) => {
  try {
    const { owner, proposalId } = req.body;
    if (!owner || !proposalId) {
      return res.status(400).json({ error: 'owner and proposalId are required' });
    }
    res.status(202).json({
      success: true,
      message: 'Revoke confirmation transaction ready.',
      data: { owner, proposalId, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/execute', contractIdMiddleware, async (req, res, next) => {
  try {
    const { proposalId } = req.body;
    if (!proposalId) return res.status(400).json({ error: 'proposalId is required' });
    logger.info(`execute proposal=${proposalId}`);
    res.status(202).json({
      success: true,
      message: 'Execute transaction ready.',
      data: { proposalId, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.post('/cancel', contractIdMiddleware, async (req, res, next) => {
  try {
    const { caller, proposalId } = req.body;
    if (!caller || !proposalId) {
      return res.status(400).json({ error: 'caller and proposalId are required' });
    }
    res.status(202).json({
      success: true,
      message: 'Cancel transaction ready.',
      data: { caller, proposalId, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

router.get('/owners', contractIdMiddleware, async (req, res, next) => {
  try {
    res.json({ success: true, data: { owners: [], contractId: req.contractId } });
  } catch (err) {
    next(err);
  }
});

router.get('/threshold', contractIdMiddleware, async (req, res, next) => {
  try {
    res.json({ success: true, data: { threshold: 0, contractId: req.contractId } });
  } catch (err) {
    next(err);
  }
});

router.get('/count', contractIdMiddleware, async (req, res, next) => {
  try {
    res.json({ success: true, data: { count: 0, contractId: req.contractId } });
  } catch (err) {
    next(err);
  }
});

module.exports = router;

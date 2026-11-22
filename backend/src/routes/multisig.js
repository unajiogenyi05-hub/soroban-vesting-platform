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
 * GET  /api/multisig/owners            — list current owners (live when MULTISIG_CONTRACT_ID is set)
 * GET  /api/multisig/threshold         — get current threshold (live when MULTISIG_CONTRACT_ID is set)
 * GET  /api/multisig/count             — proposal count
 */

const { Router } = require('express');
const logger = require('../logger');
const { validateStellarAddress } = require('../services/stellar');

const router = Router();

function requireAddress(res, value, field) {
  if (!value) {
    res.status(400).json({ error: `${field} is required` });
    return false;
  }
  if (!validateStellarAddress(value)) {
    res.status(400).json({ error: `invalid Stellar address: ${field}` });
    return false;
  }
  return true;
}

/**
 * Parse the ScVal returned by simulateContractCall into plain JS values.
 * Tolerates both host ScVal instances (`.u32()` / `.vec()` / `.address()`)
 * and raw values so the logic stays testable against a mocked service.
 */
function parseScVal(returnValue) {
  if (returnValue == null) return undefined;
  try {
    if (typeof returnValue.u32 === 'function') return returnValue.u32();
    if (typeof returnValue.vec === 'function') {
      return returnValue.vec().map((item) => {
        if (typeof item.address === 'function') return item.address().toString();
        return item;
      });
    }
  } catch (err) {
    // Fall through to returning the raw value below.
  }
  return returnValue;
}

router.post('/proposal', contractIdMiddleware, async (req, res, next) => {
  try {
    const { proposer, description } = req.body;
    if (!proposer || !description) {
      return res.status(400).json({ error: 'proposer and description are required' });
    }
    if (!requireAddress(res, proposer, 'proposer')) return;
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
    if (!requireAddress(res, owner, 'owner')) return;
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
    if (!requireAddress(res, owner, 'owner')) return;
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
    if (!requireAddress(res, caller, 'caller')) return;
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
    if (HAS_LIVE_CONTRACT) {
      const sim = await simulateContractCall({
        contractId: req.contractId,
        method: 'get_owners',
        args: [],
      });
      const parsed = parseScVal(sim.result?.returnValue);
      const owners = Array.isArray(parsed) ? parsed : [];
      return res.json({ success: true, data: { owners, contractId: req.contractId } });
    }
    res.json({ success: true, data: { owners: [], contractId: req.contractId } });
  } catch (err) {
    next(err);
  }
});

router.get('/threshold', contractIdMiddleware, async (req, res, next) => {
  try {
    if (HAS_LIVE_CONTRACT) {
      const sim = await simulateContractCall({
        contractId: req.contractId,
        method: 'get_threshold',
        args: [],
      });
      const parsed = parseScVal(sim.result?.returnValue);
      const threshold = typeof parsed === 'number' ? parsed : 0;
      return res.json({ success: true, data: { threshold, contractId: req.contractId } });
    }
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

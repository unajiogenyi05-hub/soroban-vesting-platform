'use strict';

/**
 * Vesting API Routes
 *
 * POST /api/vesting/schedule          — create a vesting schedule
 * GET  /api/vesting/schedule/:id      — get schedule details
 * GET  /api/vesting/claimable/:id     — get claimable amount
 * POST /api/vesting/claim             — claim vested tokens
 * POST /api/vesting/revoke            — revoke a schedule (admin)
 * POST /api/vesting/pause             — pause contract (admin)
 * POST /api/vesting/unpause           — unpause contract (admin)
 * GET  /api/vesting/beneficiary/:addr — list schedules for a beneficiary
 * GET  /api/vesting/count             — total schedule count
 */

const { Router } = require('express');
const { Keypair, nativeToScVal, scValToNative } = require('@stellar/stellar-sdk');
const logger = require('../logger');
const { simulateContractCall, validateStellarAddress } = require('../services/stellar');

const router = Router();

const CONTRACT_ID = process.env.VESTING_CONTRACT_ID || '';

// ── Helpers ──────────────────────────────────────────────────────────────────

function contractIdMiddleware(req, res, next) {
  const id = req.body?.contractId || req.query?.contractId || CONTRACT_ID;
  if (!id) {
    return res.status(400).json({ error: 'contractId is required' });
  }
  req.contractId = id;
  next();
}

// ── Routes ───────────────────────────────────────────────────────────────────

/**
 * GET /api/vesting/schedule/:id
 * Returns on-chain schedule data (simulated read).
 */
router.get('/schedule/:id', contractIdMiddleware, async (req, res, next) => {
  try {
    const rawId = req.params.id;
    const isValidPositiveInteger = /^\d+$/.test(rawId) && BigInt(rawId) > 0n;

    if (!isValidPositiveInteger) {
      return res.status(400).json({ error: 'scheduleId must be a positive integer' });
    }

    const scheduleId = BigInt(rawId);
    logger.info(`get_schedule id=${scheduleId}`);

    // In a full implementation this would call simulateContractCall.
    // We return a documented stub so the frontend can be built against this shape.
    res.json({
      success: true,
      data: {
        id: Number(scheduleId),
        contractId: req.contractId,
        note: 'Connect VESTING_CONTRACT_ID and SOURCE_SECRET_KEY to read live data.',
      },
    });
  } catch (err) {
    next(err);
  }
});

/**
 * GET /api/vesting/claimable/:id
 */
router.get('/claimable/:id', async (req, res, next) => {
  try {
    const rawId = req.params.id;
    if (!/^\d+$/.test(rawId)) {
      return res.status(400).json({ error: 'scheduleId must be a positive u64 integer' });
    }

    const scheduleId = BigInt(rawId);
    if (scheduleId <= 0n || scheduleId > 18446744073709551615n) {
      return res.status(400).json({ error: 'scheduleId must be a positive u64 integer' });
    }

    const contractId = process.env.VESTING_CONTRACT_ID || '';
    const sourceSecret = process.env.SOURCE_SECRET_KEY || '';

    if (contractId && sourceSecret) {
      const sourceKeypair = Keypair.fromSecret(sourceSecret);
      const simResult = await simulateContractCall({
        contractId,
        method: 'get_claimable',
        args: [nativeToScVal(scheduleId, { type: 'u64' })],
        sourceKeypair,
      });
      const claimableAmount = scValToNative(simResult.result.retval);

      return res.json({
        success: true,
        data: {
          scheduleId: Number(scheduleId),
          claimableAmount: claimableAmount.toString(),
          contractId,
        },
      });
    }

    res.json({
      success: true,
      data: {
        scheduleId: Number(scheduleId),
        claimableAmount: 0,
        note: 'Set VESTING_CONTRACT_ID to query live contract.',
      },
    });
  } catch (err) {
    next(err);
  }
});

/**
 * POST /api/vesting/schedule
 * Body: { contractId, from, beneficiary, tokenAddress, totalAmount,
 *         startTime, cliffDuration, totalDuration, signerKey }
 */
router.post('/schedule', contractIdMiddleware, async (req, res, next) => {
  try {
    const {
      from, beneficiary, tokenAddress, totalAmount,
      startTime, cliffDuration, totalDuration,
    } = req.body;

    if (!from || !beneficiary || !tokenAddress || !totalAmount) {
      return res.status(400).json({ error: 'Missing required fields' });
    }
    if (!validateStellarAddress(from)) {
      return res.status(400).json({ error: 'invalid Stellar address: from' });
    }
    if (!validateStellarAddress(beneficiary)) {
      return res.status(400).json({ error: 'invalid Stellar address: beneficiary' });
    }
    if (!validateStellarAddress(tokenAddress)) {
      return res.status(400).json({ error: 'invalid Stellar address: tokenAddress' });
    }
    if (Number(totalAmount) <= 0) {
      return res.status(400).json({ error: 'totalAmount must be positive' });
    }
    if (Number(cliffDuration) > Number(totalDuration)) {
      return res.status(400).json({ error: 'cliffDuration cannot exceed totalDuration' });
    }

    logger.info(`create_schedule from=${from} beneficiary=${beneficiary} amount=${totalAmount}`);

    res.status(202).json({
      success: true,
      message: 'Schedule creation queued. Sign and submit the transaction.',
      data: {
        contractId: req.contractId,
        from,
        beneficiary,
        tokenAddress,
        totalAmount,
        startTime: startTime || Math.floor(Date.now() / 1000),
        cliffDuration: cliffDuration || 0,
        totalDuration,
      },
    });
  } catch (err) {
    next(err);
  }
});

/**
 * POST /api/vesting/claim
 * Body: { contractId, scheduleId, beneficiary }
 */
router.post('/claim', contractIdMiddleware, async (req, res, next) => {
  try {
    const { scheduleId, beneficiary } = req.body;
    if (!scheduleId || !beneficiary) {
      return res.status(400).json({ error: 'scheduleId and beneficiary are required' });
    }
    if (!validateStellarAddress(beneficiary)) {
      return res.status(400).json({ error: 'invalid Stellar address: beneficiary' });
    }
    logger.info(`claim scheduleId=${scheduleId} by=${beneficiary}`);
    res.status(202).json({
      success: true,
      message: 'Claim transaction ready for signing.',
      data: { scheduleId, beneficiary, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

/**
 * POST /api/vesting/revoke
 * Body: { contractId, scheduleId, recipient, adminKey }
 */
router.post('/revoke', contractIdMiddleware, async (req, res, next) => {
  try {
    const { scheduleId, recipient } = req.body;
    if (!scheduleId || !recipient) {
      return res.status(400).json({ error: 'scheduleId and recipient are required' });
    }
    if (!validateStellarAddress(recipient)) {
      return res.status(400).json({ error: 'invalid Stellar address: recipient' });
    }
    logger.info(`revoke scheduleId=${scheduleId} recipient=${recipient}`);
    res.status(202).json({
      success: true,
      message: 'Revoke transaction ready for signing.',
      data: { scheduleId, recipient, contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

/**
 * POST /api/vesting/pause
 */
router.post('/pause', contractIdMiddleware, async (req, res, next) => {
  try {
    logger.info(`pause contractId=${req.contractId}`);
    res.status(202).json({ success: true, message: 'Pause transaction ready.' });
  } catch (err) {
    next(err);
  }
});

/**
 * POST /api/vesting/unpause
 */
router.post('/unpause', contractIdMiddleware, async (req, res, next) => {
  try {
    logger.info(`unpause contractId=${req.contractId}`);
    res.status(202).json({ success: true, message: 'Unpause transaction ready.' });
  } catch (err) {
    next(err);
  }
});

/**
 * GET /api/vesting/beneficiary/:addr
 */
router.get('/beneficiary/:addr', contractIdMiddleware, async (req, res, next) => {
  try {
    const { addr } = req.params;
    logger.info(`get_beneficiary_schedules addr=${addr}`);
    res.json({
      success: true,
      data: { beneficiary: addr, scheduleIds: [], contractId: req.contractId },
    });
  } catch (err) {
    next(err);
  }
});

/**
 * GET /api/vesting/count
 */
router.get('/count', contractIdMiddleware, async (req, res, next) => {
  try {
    res.json({ success: true, data: { count: 0, contractId: req.contractId } });
  } catch (err) {
    next(err);
  }
});

module.exports = router;

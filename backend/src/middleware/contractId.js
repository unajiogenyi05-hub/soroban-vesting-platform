'use strict';

/**
 * Middleware factory that resolves the contract ID for a route.
 *
 * Resolution order:
 *   1. `req.body.contractId`
 *   2. `req.query.contractId`
 *   3. `envContractId` (typically `process.env.<CONTRACT>_CONTRACT_ID`)
 *
 * The resolved id is stored on `req.contractId` and a `400` is returned
 * when no id can be resolved. Centralising this logic here keeps the
 * vesting, token and multisig route files DRY.
 *
 * @param {string} envContractId - Fallback contract ID from the environment
 * @returns {Function} Express middleware `(req, res, next)`
 */
module.exports =
  (envContractId) =>
  (req, res, next) => {
    const id = req.body?.contractId || req.query?.contractId || envContractId;
    if (!id) {
      return res.status(400).json({ error: 'contractId is required' });
    }
    req.contractId = id;
    next();
  };

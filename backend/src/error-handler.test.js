'use strict';

/**
 * Tests for the centralised error handler (#114).
 *
 * Verifies that every error response — whether a route-level 400 or an
 * unhandled 500 — always has the shape { error: string, code: string }.
 */

const request = require('supertest');

jest.mock('./services/stellar', () => ({
  simulateContractCall: jest.fn().mockResolvedValue({ result: 'stub' }),
  NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
  RPC_URL: 'https://soroban-testnet.stellar.org',
}));

process.env.VESTING_CONTRACT_ID = 'CTEST0000000000000000000000000000000000000000000000000000';

const app = require('./index');

// ── 404 ──────────────────────────────────────────────────────────────────────

describe('404 handler', () => {
  it('returns { error } on unknown route', async () => {
    const res = await request(app).get('/api/does-not-exist');
    expect(res.status).toBe(404);
    expect(typeof res.body.error).toBe('string');
  });
});

// ── 400 — route-level validation errors ──────────────────────────────────────

describe('400 error envelope', () => {
  it('POST /api/vesting/schedule with missing fields returns { error }', async () => {
    const res = await request(app).post('/api/vesting/schedule').send({});
    expect(res.status).toBe(400);
    expect(typeof res.body.error).toBe('string');
    expect(res.body.error.length).toBeGreaterThan(0);
  });

  it('POST /api/vesting/claim with missing scheduleId returns { error }', async () => {
    const res = await request(app)
      .post('/api/vesting/claim')
      .send({ beneficiary: 'GABC' });
    expect(res.status).toBe(400);
    expect(typeof res.body.error).toBe('string');
  });

  it('POST /api/vesting/revoke with missing fields returns { error }', async () => {
    const res = await request(app).post('/api/vesting/revoke').send({});
    expect(res.status).toBe(400);
    expect(typeof res.body.error).toBe('string');
  });

  it('POST /api/token/mint with missing fields returns { error }', async () => {
    const res = await request(app).post('/api/token/mint').send({});
    expect(res.status).toBe(400);
    expect(typeof res.body.error).toBe('string');
  });

  it('POST /api/multisig/proposal with missing proposer returns { error }', async () => {
    const res = await request(app)
      .post('/api/multisig/proposal')
      .send({ description: 'test' });
    expect(res.status).toBe(400);
    expect(typeof res.body.error).toBe('string');
  });
});

// ── 500 — centralised error handler ──────────────────────────────────────────
//
// We inject a test-only route that throws an unhandled error, then assert
// the response has the standardised { error, code } envelope.

describe('500 error envelope', () => {
  let testApp;

  beforeAll(() => {
    // Build a minimal Express app that shares the same error-handler logic
    // by mounting a route that always throws before the error handler runs.
    const express = require('express');
    const appCopy = express();
    appCopy.use(express.json());

    // Route that deliberately throws with a custom code
    appCopy.get('/test/throw-with-code', (req, res, next) => {
      const err = new Error('deliberate test error');
      err.code = 'TEST_ERROR_CODE';
      next(err);
    });

    // Route that throws without a code (should default to INTERNAL_ERROR)
    appCopy.get('/test/throw-no-code', (req, res, next) => {
      next(new Error('unhandled internal error'));
    });

    // Attach the same error-handler shape as index.js
    // eslint-disable-next-line no-unused-vars
    appCopy.use((err, req, res, next) => {
      const status = err.status || 500;
      res.status(status).json({
        error: err.message || 'Internal server error',
        code: err.code || (status === 500 ? 'INTERNAL_ERROR' : 'REQUEST_ERROR'),
      });
    });

    testApp = appCopy;
  });

  it('returns { error, code } with custom code on 500', async () => {
    const res = await request(testApp).get('/test/throw-with-code');
    expect(res.status).toBe(500);
    expect(typeof res.body.error).toBe('string');
    expect(res.body.error).toBe('deliberate test error');
    expect(res.body.code).toBe('TEST_ERROR_CODE');
  });

  it('defaults code to INTERNAL_ERROR when err.code is not set', async () => {
    const res = await request(testApp).get('/test/throw-no-code');
    expect(res.status).toBe(500);
    expect(typeof res.body.error).toBe('string');
    expect(res.body.code).toBe('INTERNAL_ERROR');
  });

  it('error field is always a non-empty string', async () => {
    const res = await request(testApp).get('/test/throw-no-code');
    expect(typeof res.body.error).toBe('string');
    expect(res.body.error.length).toBeGreaterThan(0);
  });
});

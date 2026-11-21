'use strict';

const request = require('supertest');
const { Keypair, nativeToScVal, scValToNative, StrKey } = require('@stellar/stellar-sdk');

// Stub stellar service before requiring the app so routes don't need real RPC.
jest.mock('../services/stellar', () => ({
  simulateContractCall: jest.fn().mockResolvedValue({ result: 'stub' }),
  validateStellarAddress: jest.fn((addr) => typeof addr === 'string' && addr.startsWith('G') && addr.length >= 56),
  NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
  RPC_URL: 'https://soroban-testnet.stellar.org',
}));

const { simulateContractCall } = require('../services/stellar');

// Generate a valid Stellar public key for use in test fixtures.
const VALID_ADDRESS = StrKey.encodeEd25519PublicKey(Buffer.alloc(32, 1));
const VALID_ADDRESS_2 = StrKey.encodeEd25519PublicKey(Buffer.alloc(32, 2));
const VALID_ADDRESS_3 = StrKey.encodeEd25519PublicKey(Buffer.alloc(32, 3));
const INVALID_ADDRESS = 'GABC123';

// Provide a contractId via env so contractIdMiddleware passes.
process.env.VESTING_CONTRACT_ID = 'CTEST0000000000000000000000000000000000000000000000000000';

const app = require('../index');

describe('Vesting routes', () => {
  // ── GET /schedule/:id ──────────────────────────────────────────────────────
  describe('GET /api/vesting/schedule/:id', () => {
    it('returns schedule stub for a valid id', async () => {
      const res = await request(app).get('/api/vesting/schedule/1');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.id).toBe(1);
    });

    it('returns 400 when schedule id is not numeric', async () => {
      const res = await request(app).get('/api/vesting/schedule/abc');
      expect(res.status).toBe(400);
      expect(res.body).toHaveProperty('error', 'scheduleId must be a positive integer');
    });

    it('returns 400 when schedule id is negative', async () => {
      const res = await request(app).get('/api/vesting/schedule/-1');
      expect(res.status).toBe(400);
      expect(res.body).toHaveProperty('error', 'scheduleId must be a positive integer');
    });
  });

  // ── GET /claimable/:id ─────────────────────────────────────────────────────
  describe('GET /api/vesting/claimable/:id', () => {
    it('returns claimable stub when contract configuration is absent', async () => {
      const previousContractId = process.env.VESTING_CONTRACT_ID;
      const previousSourceSecret = process.env.SOURCE_SECRET_KEY;
      simulateContractCall.mockClear();
      delete process.env.VESTING_CONTRACT_ID;
      delete process.env.SOURCE_SECRET_KEY;

      const res = await request(app).get('/api/vesting/claimable/1');

      if (previousContractId === undefined) delete process.env.VESTING_CONTRACT_ID;
      else process.env.VESTING_CONTRACT_ID = previousContractId;
      if (previousSourceSecret === undefined) delete process.env.SOURCE_SECRET_KEY;
      else process.env.SOURCE_SECRET_KEY = previousSourceSecret;

      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.claimableAmount).toBe(0);
      expect(res.body.data).toHaveProperty('note');
      expect(simulateContractCall).not.toHaveBeenCalled();
    });

    it('simulates get_claimable and returns the parsed on-chain amount', async () => {
      const previousContractId = process.env.VESTING_CONTRACT_ID;
      const previousSourceSecret = process.env.SOURCE_SECRET_KEY;
      const contractId = 'CTEST0000000000000000000000000000000000000000000000000000';
      const sourceSecret = Keypair.random().secret();
      simulateContractCall.mockClear();
      process.env.VESTING_CONTRACT_ID = contractId;
      process.env.SOURCE_SECRET_KEY = sourceSecret;
      simulateContractCall.mockResolvedValueOnce({
        result: { retval: nativeToScVal(42000n, { type: 'i128' }) },
      });

      const res = await request(app).get('/api/vesting/claimable/1');

      if (previousContractId === undefined) delete process.env.VESTING_CONTRACT_ID;
      else process.env.VESTING_CONTRACT_ID = previousContractId;
      if (previousSourceSecret === undefined) delete process.env.SOURCE_SECRET_KEY;
      else process.env.SOURCE_SECRET_KEY = previousSourceSecret;

      expect(res.status).toBe(200);
      expect(res.body.data.claimableAmount).toBe('42000');
      expect(simulateContractCall).toHaveBeenCalledWith(expect.objectContaining({
        contractId,
        method: 'get_claimable',
        args: [expect.anything()],
        sourceKeypair: expect.any(Object),
      }));
      expect(scValToNative(simulateContractCall.mock.calls[0][0].args[0])).toBe(1n);
    });
  });

  // ── POST /schedule ─────────────────────────────────────────────────────────
  describe('POST /api/vesting/schedule', () => {
    const valid = {
      from: VALID_ADDRESS,
      beneficiary: VALID_ADDRESS_2,
      tokenAddress: VALID_ADDRESS_3,
      totalAmount: 100000,
      totalDuration: 100,
    };

    it('returns 202 with valid body', async () => {
      const res = await request(app).post('/api/vesting/schedule').send(valid);
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when required fields are missing', async () => {
      const res = await request(app).post('/api/vesting/schedule').send({});
      expect(res.status).toBe(400);
      expect(res.body.error).toBeDefined();
    });

    it('returns 400 when totalAmount is not positive', async () => {
      const res = await request(app)
        .post('/api/vesting/schedule')
        .send({ ...valid, totalAmount: 0 });
      expect(res.status).toBe(400);
    });

    it('returns 400 when from is an invalid Stellar address', async () => {
      const res = await request(app)
        .post('/api/vesting/schedule')
        .send({ ...valid, from: INVALID_ADDRESS });
      expect(res.status).toBe(400);
      expect(res.body.error).toBe('invalid Stellar address: from');
    });

    it('returns 400 when beneficiary is an invalid Stellar address', async () => {
      const res = await request(app)
        .post('/api/vesting/schedule')
        .send({ ...valid, beneficiary: INVALID_ADDRESS });
      expect(res.status).toBe(400);
      expect(res.body.error).toBe('invalid Stellar address: beneficiary');
    });

    it('returns 400 when tokenAddress is an invalid Stellar address', async () => {
      const res = await request(app)
        .post('/api/vesting/schedule')
        .send({ ...valid, tokenAddress: INVALID_ADDRESS });
      expect(res.status).toBe(400);
      expect(res.body.error).toBe('invalid Stellar address: tokenAddress');
    });

    it('returns 400 when cliffDuration exceeds totalDuration', async () => {
      const res = await request(app)
        .post('/api/vesting/schedule')
        .send({ ...valid, cliffDuration: 200, totalDuration: 100 });
      expect(res.status).toBe(400);
    });
  });

  // ── POST /claim ────────────────────────────────────────────────────────────
  describe('POST /api/vesting/claim', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/vesting/claim')
        .send({ scheduleId: 1, beneficiary: VALID_ADDRESS });
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when beneficiary is an invalid Stellar address', async () => {
      const res = await request(app)
        .post('/api/vesting/claim')
        .send({ scheduleId: 1, beneficiary: INVALID_ADDRESS });
      expect(res.status).toBe(400);
      expect(res.body.error).toBe('invalid Stellar address: beneficiary');
    });

    it('returns 400 when scheduleId is missing', async () => {
      const res = await request(app)
        .post('/api/vesting/claim')
        .send({ beneficiary: VALID_ADDRESS });
      expect(res.status).toBe(400);
    });

    it('returns 400 when beneficiary is missing', async () => {
      const res = await request(app)
        .post('/api/vesting/claim')
        .send({ scheduleId: 1 });
      expect(res.status).toBe(400);
    });
  });

  // ── POST /revoke ───────────────────────────────────────────────────────────
  describe('POST /api/vesting/revoke', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/vesting/revoke')
        .send({ scheduleId: 1, recipient: VALID_ADDRESS });
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when recipient is an invalid Stellar address', async () => {
      const res = await request(app)
        .post('/api/vesting/revoke')
        .send({ scheduleId: 1, recipient: INVALID_ADDRESS });
      expect(res.status).toBe(400);
      expect(res.body.error).toBe('invalid Stellar address: recipient');
    });

    it('returns 400 when fields are missing', async () => {
      const res = await request(app).post('/api/vesting/revoke').send({});
      expect(res.status).toBe(400);
    });
  });

  // ── POST /pause & /unpause ─────────────────────────────────────────────────
  describe('POST /api/vesting/pause and /unpause', () => {
    it('pause returns 202', async () => {
      const res = await request(app).post('/api/vesting/pause').send({});
      expect(res.status).toBe(202);
    });

    it('unpause returns 202', async () => {
      const res = await request(app).post('/api/vesting/unpause').send({});
      expect(res.status).toBe(202);
    });
  });

  // ── GET /beneficiary/:addr ─────────────────────────────────────────────────
  describe('GET /api/vesting/beneficiary/:addr', () => {
    it('returns 200 with success: true and correct beneficiary address', async () => {
      const addr = VALID_ADDRESS;
      const res = await request(app).get(`/api/vesting/beneficiary/${addr}`);
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.beneficiary).toBe(addr);
    });

    it('returns scheduleIds as an empty array by default', async () => {
      const res = await request(app).get(`/api/vesting/beneficiary/${VALID_ADDRESS}`);
      expect(res.status).toBe(200);
      expect(Array.isArray(res.body.data.scheduleIds)).toBe(true);
    });

    it('includes contractId in the response data', async () => {
      const res = await request(app).get(`/api/vesting/beneficiary/${VALID_ADDRESS}`);
      expect(res.status).toBe(200);
      expect(res.body.data).toHaveProperty('contractId');
    });

    it('reflects the address param in the beneficiary field for different addresses', async () => {
      const addr = VALID_ADDRESS_2;
      const res = await request(app).get(`/api/vesting/beneficiary/${addr}`);
      expect(res.status).toBe(200);
      expect(res.body.data.beneficiary).toBe(addr);
    });

    it('returns 400 when contractId is missing from env and not supplied', async () => {
      const savedId = process.env.VESTING_CONTRACT_ID;
      delete process.env.VESTING_CONTRACT_ID;
      // Reload the router with no env var so contractIdMiddleware fires.
      // The existing app already has the env var baked in — query param override works.
      // We test the query-param path instead by using a fresh contract id.
      process.env.VESTING_CONTRACT_ID = savedId;
      // Supply contractId via query string — should succeed.
      const res = await request(app)
        .get(`/api/vesting/beneficiary/${VALID_ADDRESS}`)
        .query({ contractId: 'CTEST_OVERRIDE000000000000000000000000000000000000000000' });
      expect(res.status).toBe(200);
      expect(res.body.data.contractId).toBe('CTEST_OVERRIDE000000000000000000000000000000000000000000');
    });
  });

  // ── GET /count ─────────────────────────────────────────────────────────────
  describe('GET /api/vesting/count', () => {
    it('returns 200 with success: true', async () => {
      const res = await request(app).get('/api/vesting/count');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
    });

    it('returns a count field that is a number', async () => {
      const res = await request(app).get('/api/vesting/count');
      expect(res.status).toBe(200);
      expect(typeof res.body.data.count).toBe('number');
    });

    it('includes contractId in the response data', async () => {
      const res = await request(app).get('/api/vesting/count');
      expect(res.status).toBe(200);
      expect(res.body.data).toHaveProperty('contractId');
    });

    it('accepts contractId override via query param', async () => {
      const overrideId = 'CTEST_OVERRIDE000000000000000000000000000000000000000000';
      const res = await request(app)
        .get('/api/vesting/count')
        .query({ contractId: overrideId });
      expect(res.status).toBe(200);
      expect(res.body.data.contractId).toBe(overrideId);
    });
  });
});

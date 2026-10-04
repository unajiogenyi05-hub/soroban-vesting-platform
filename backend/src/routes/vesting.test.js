'use strict';

const request = require('supertest');

// Stub stellar service before requiring the app so routes don't need real RPC.
jest.mock('../services/stellar', () => ({
  simulateContractCall: jest.fn().mockResolvedValue({ result: 'stub' }),
  NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
  RPC_URL: 'https://soroban-testnet.stellar.org',
}));

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
  });

  // ── GET /claimable/:id ─────────────────────────────────────────────────────
  describe('GET /api/vesting/claimable/:id', () => {
    it('returns claimable stub', async () => {
      const res = await request(app).get('/api/vesting/claimable/1');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toHaveProperty('claimableAmount');
    });
  });

  // ── POST /schedule ─────────────────────────────────────────────────────────
  describe('POST /api/vesting/schedule', () => {
    const valid = {
      from: 'GABC1234567890123456789012345678901234567890123456789012',
      beneficiary: 'GDEF1234567890123456789012345678901234567890123456789012',
      tokenAddress: 'GHIJ1234567890123456789012345678901234567890123456789012',
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
        .send({ scheduleId: 1, beneficiary: 'GABC123' });
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when scheduleId is missing', async () => {
      const res = await request(app)
        .post('/api/vesting/claim')
        .send({ beneficiary: 'GABC123' });
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
        .send({ scheduleId: 1, recipient: 'GABC123' });
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
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
    it('returns schedule ids list', async () => {
      const res = await request(app).get('/api/vesting/beneficiary/GABC123');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(Array.isArray(res.body.data.scheduleIds)).toBe(true);
    });
  });

  // ── GET /count ─────────────────────────────────────────────────────────────
  describe('GET /api/vesting/count', () => {
    it('returns count', async () => {
      const res = await request(app).get('/api/vesting/count');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toHaveProperty('count');
    });
  });
});

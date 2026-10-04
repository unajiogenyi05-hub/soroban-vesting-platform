'use strict';

const request = require('supertest');

jest.mock('../services/stellar', () => ({
  simulateContractCall: jest.fn().mockResolvedValue({ result: 'stub' }),
  NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
  RPC_URL: 'https://soroban-testnet.stellar.org',
}));

process.env.MULTISIG_CONTRACT_ID = 'CTEST0000000000000000000000000000000000000000000000000000';

const app = require('../index');

describe('Multisig routes', () => {
  describe('POST /api/multisig/proposal', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/multisig/proposal')
        .send({ proposer: 'GABC', description: 'create_schedule' });
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when proposer missing', async () => {
      const res = await request(app)
        .post('/api/multisig/proposal')
        .send({ description: 'test' });
      expect(res.status).toBe(400);
    });

    it('returns 400 when description missing', async () => {
      const res = await request(app)
        .post('/api/multisig/proposal')
        .send({ proposer: 'GABC' });
      expect(res.status).toBe(400);
    });
  });

  describe('GET /api/multisig/proposal/:id', () => {
    it('returns proposal stub', async () => {
      const res = await request(app).get('/api/multisig/proposal/1');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.id).toBe(1);
    });
  });

  describe('POST /api/multisig/confirm', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/multisig/confirm')
        .send({ owner: 'GABC', proposalId: 1 });
      expect(res.status).toBe(202);
    });

    it('returns 400 when fields missing', async () => {
      const res = await request(app).post('/api/multisig/confirm').send({});
      expect(res.status).toBe(400);
    });
  });

  describe('POST /api/multisig/execute', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/multisig/execute')
        .send({ proposalId: 1 });
      expect(res.status).toBe(202);
    });

    it('returns 400 when proposalId missing', async () => {
      const res = await request(app).post('/api/multisig/execute').send({});
      expect(res.status).toBe(400);
    });
  });

  describe('POST /api/multisig/cancel', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/multisig/cancel')
        .send({ caller: 'GABC', proposalId: 1 });
      expect(res.status).toBe(202);
    });

    it('returns 400 when fields missing', async () => {
      const res = await request(app).post('/api/multisig/cancel').send({});
      expect(res.status).toBe(400);
    });
  });

  describe('GET /api/multisig/owners', () => {
    it('returns owners list', async () => {
      const res = await request(app).get('/api/multisig/owners');
      expect(res.status).toBe(200);
      expect(Array.isArray(res.body.data.owners)).toBe(true);
    });
  });

  describe('GET /api/multisig/threshold', () => {
    it('returns threshold', async () => {
      const res = await request(app).get('/api/multisig/threshold');
      expect(res.status).toBe(200);
      expect(res.body.data).toHaveProperty('threshold');
    });
  });
});

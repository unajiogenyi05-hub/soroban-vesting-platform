'use strict';

const request = require('supertest');

const VALID_ADDRESS = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';

jest.mock('../services/stellar', () => ({
  simulateContractCall: jest.fn().mockResolvedValue({ result: 'stub' }),
  validateStellarAddress: jest.fn((addr) => typeof addr === 'string' && addr.startsWith('G') && addr.length > 2 && !['GOWNER'].includes(addr)),
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
        .send({ proposer: VALID_ADDRESS, description: 'create_schedule' });
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
        .send({ proposer: VALID_ADDRESS });
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
        .send({ owner: VALID_ADDRESS, proposalId: 1 });
      expect(res.status).toBe(202);
    });

    it('returns 400 when fields missing', async () => {
      const res = await request(app).post('/api/multisig/confirm').send({});
      expect(res.status).toBe(400);
    });

    it('returns 400 for invalid owner address', async () => {
      const res = await request(app)
        .post('/api/multisig/confirm')
        .send({ owner: 'GOWNER', proposalId: 1 });
      expect(res.status).toBe(400);
      expect(res.body.error).toMatch(/invalid Stellar address: owner/);
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
        .send({ caller: VALID_ADDRESS, proposalId: 1 });
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

  describe('POST /api/multisig/revoke-confirm', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/multisig/revoke-confirm')
        .send({ owner: VALID_ADDRESS, proposalId: 1 });
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when owner is missing', async () => {
      const res = await request(app)
        .post('/api/multisig/revoke-confirm')
        .send({ proposalId: 1 });
      expect(res.status).toBe(400);
    });

    it('returns 400 when proposalId is missing', async () => {
      const res = await request(app)
        .post('/api/multisig/revoke-confirm')
        .send({ owner: VALID_ADDRESS });
      expect(res.status).toBe(400);
    });

    it('returns 400 when body is empty', async () => {
      const res = await request(app)
        .post('/api/multisig/revoke-confirm')
        .send({});
      expect(res.status).toBe(400);
    });

    it('includes owner and proposalId in response data', async () => {
      const res = await request(app)
        .post('/api/multisig/revoke-confirm')
        .send({ owner: VALID_ADDRESS, proposalId: 42 });
      expect(res.status).toBe(202);
      expect(res.body.data.owner).toBe(VALID_ADDRESS);
      expect(res.body.data.proposalId).toBe(42);
    });

    it('returns 400 for invalid owner address', async () => {
      const res = await request(app)
        .post('/api/multisig/revoke-confirm')
        .send({ owner: 'not-a-stellar-address', proposalId: 42 });
      expect(res.status).toBe(400);
      expect(res.body.error).toMatch(/invalid Stellar address: owner/);
    });
  });
});

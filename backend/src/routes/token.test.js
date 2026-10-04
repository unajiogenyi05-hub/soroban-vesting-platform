'use strict';

const request = require('supertest');

jest.mock('../services/stellar', () => ({
  simulateContractCall: jest.fn().mockResolvedValue({ result: 'stub' }),
  NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
  RPC_URL: 'https://soroban-testnet.stellar.org',
}));

process.env.TOKEN_CONTRACT_ID = 'CTEST0000000000000000000000000000000000000000000000000000';

const app = require('../index');

describe('Token routes', () => {
  describe('GET /api/token/info', () => {
    it('returns token info stub', async () => {
      const res = await request(app).get('/api/token/info');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toHaveProperty('symbol');
    });
  });

  describe('GET /api/token/balance/:addr', () => {
    it('returns balance stub', async () => {
      const res = await request(app).get('/api/token/balance/GABC123');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toHaveProperty('balance');
    });
  });

  describe('GET /api/token/allowance', () => {
    it('returns allowance stub with owner and spender', async () => {
      const res = await request(app).get('/api/token/allowance?owner=GABC&spender=GDEF');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when query params missing', async () => {
      const res = await request(app).get('/api/token/allowance');
      expect(res.status).toBe(400);
    });
  });

  describe('POST /api/token/mint', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app).post('/api/token/mint').send({ to: 'GABC', amount: 1000 });
      expect(res.status).toBe(202);
      expect(res.body.success).toBe(true);
    });

    it('returns 400 when to or amount missing', async () => {
      const res = await request(app).post('/api/token/mint').send({});
      expect(res.status).toBe(400);
    });

    it('returns 400 when amount is zero or negative', async () => {
      const res = await request(app).post('/api/token/mint').send({ to: 'GABC', amount: 0 });
      expect(res.status).toBe(400);
    });
  });

  describe('POST /api/token/transfer', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/token/transfer')
        .send({ from: 'GABC', to: 'GDEF', amount: 500 });
      expect(res.status).toBe(202);
    });

    it('returns 400 when fields missing', async () => {
      const res = await request(app).post('/api/token/transfer').send({ from: 'GABC' });
      expect(res.status).toBe(400);
    });
  });

  describe('POST /api/token/approve', () => {
    it('returns 202 with valid body', async () => {
      const res = await request(app)
        .post('/api/token/approve')
        .send({ owner: 'GABC', spender: 'GDEF', amount: 1000 });
      expect(res.status).toBe(202);
    });

    it('returns 400 when fields missing', async () => {
      const res = await request(app).post('/api/token/approve').send({});
      expect(res.status).toBe(400);
    });
  });

  describe('POST /api/token/pause and /unpause', () => {
    it('pause returns 202', async () => {
      const res = await request(app).post('/api/token/pause').send({});
      expect(res.status).toBe(202);
    });

    it('unpause returns 202', async () => {
      const res = await request(app).post('/api/token/unpause').send({});
      expect(res.status).toBe(202);
    });
  });
});

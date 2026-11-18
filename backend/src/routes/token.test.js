'use strict';

const request = require('supertest');

// ── Mock the stellar service ─────────────────────────────────────────────────
// Default: simulateContractCall resolves with a stub i128 value of 42000.
// Individual tests can override via mockResolvedValueOnce.
const mockSimulate = jest.fn().mockResolvedValue({
  result: {
    retval: {
      // scValToNative will be called on this; we mock the module so it returns
      // BigInt(42000) directly.
    },
  },
});

jest.mock('../services/stellar', () => ({
  simulateContractCall: mockSimulate,
  validateStellarAddress: jest.fn((addr) => typeof addr === 'string' && addr.startsWith('G') && !['not-a-stellar-address', 'GABC123'].includes(addr)),
  NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
  RPC_URL: 'https://soroban-testnet.stellar.org',
}));

// Mock stellar-sdk so Keypair.fromSecret and scValToNative are controlled.
jest.mock('@stellar/stellar-sdk', () => ({
  Keypair: {
    fromSecret: jest.fn().mockReturnValue({ publicKey: () => 'GPUBKEY' }),
  },
  nativeToScVal: jest.fn().mockReturnValue('mockScVal'),
  scValToNative: jest.fn().mockReturnValue(BigInt(42000)),
  SorobanRpc: { Server: jest.fn() },
  Networks: { TESTNET: 'Test SDF Network ; September 2015', PUBLIC: 'Public Global Stellar Network ; September 2015' },
  TransactionBuilder: jest.fn(),
  BASE_FEE: '100',
  Contract: jest.fn(),
}));

// ── Env setup ─────────────────────────────────────────────────────────────────
// Set TOKEN_CONTRACT_ID so the middleware doesn't return 400.
process.env.TOKEN_CONTRACT_ID = 'CTEST0000000000000000000000000000000000000000000000000000';

const app = require('../index');

// Valid G… Stellar address used in balance tests (56 chars: G + 55 base-32 chars)
const VALID_ADDR = 'GBMOMCDFID6S4ALFZP36ZIVA3RXDEZ6K5IAD5K3IQ3UNYTZHQN4BPMEA';

describe('Token routes', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    // Re-establish default mock return for each test
    mockSimulate.mockResolvedValue({
      result: { retval: {} },
    });
    const { scValToNative } = require('@stellar/stellar-sdk');
    scValToNative.mockReturnValue(BigInt(42000));
  });

  // ── info ────────────────────────────────────────────────────────────────────

  describe('GET /api/token/info', () => {
    it('returns token info stub', async () => {
      const res = await request(app).get('/api/token/info');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toHaveProperty('symbol');
    });
  });

  // ── balance — stub fallback ─────────────────────────────────────────────────

  describe('GET /api/token/balance/:addr — stub fallback', () => {
    it('returns stub (balance: 0) when SOURCE_SECRET_KEY is not set', async () => {
      delete process.env.SOURCE_SECRET_KEY;
      const res = await request(app).get(`/api/token/balance/${VALID_ADDR}`);
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.balance).toBe(0);
      expect(mockSimulate).not.toHaveBeenCalled();
    });

    it('returns stub with note field when SOURCE_SECRET_KEY is absent', async () => {
      delete process.env.SOURCE_SECRET_KEY;
      const res = await request(app).get(`/api/token/balance/${VALID_ADDR}`);
      expect(res.body.data).toHaveProperty('note');
    });
  });

  // ── balance — live simulateContractCall ─────────────────────────────────────

  describe('GET /api/token/balance/:addr — live call', () => {
    beforeEach(() => {
      process.env.SOURCE_SECRET_KEY = 'SDQFQ4RQMKL7K73TKA3Z6IUXWTUY3XMLHYQH7G4MREIOAG6GZFGIKORJ';
    });

    afterEach(() => {
      delete process.env.SOURCE_SECRET_KEY;
    });

    it('calls simulateContractCall and returns parsed balance', async () => {
      const res = await request(app).get(`/api/token/balance/${VALID_ADDR}`);
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      // scValToNative returns BigInt(42000); route converts to string
      expect(res.body.data.balance).toBe('42000');
      expect(mockSimulate).toHaveBeenCalledTimes(1);
      expect(mockSimulate).toHaveBeenCalledWith(
        expect.objectContaining({
          contractId: process.env.TOKEN_CONTRACT_ID,
          method: 'balance',
        })
      );
    });

    it('returns 400 for an invalid Stellar address', async () => {
      const res = await request(app).get('/api/token/balance/not-a-stellar-address');
      expect(res.status).toBe(400);
      expect(res.body).toHaveProperty('error');
      expect(mockSimulate).not.toHaveBeenCalled();
    });

    it('propagates errors from simulateContractCall to the error handler', async () => {
      mockSimulate.mockRejectedValueOnce(new Error('RPC error'));
      const res = await request(app).get(`/api/token/balance/${VALID_ADDR}`);
      // Error handler returns 500
      expect(res.status).toBe(500);
    });
  });

  // ── allowance ───────────────────────────────────────────────────────────────

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

  // ── mint ────────────────────────────────────────────────────────────────────

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

  // ── transfer ────────────────────────────────────────────────────────────────

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

  // ── approve ─────────────────────────────────────────────────────────────────

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

  // ── pause / unpause ─────────────────────────────────────────────────────────

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

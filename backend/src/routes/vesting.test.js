'use strict';

const request = require('supertest');

// Stub stellar service before requiring the app so routes don't need real RPC.
const mockSimulate = jest.fn().mockResolvedValue({ result: { retval: {} } });

jest.mock('../services/stellar', () => ({
  simulateContractCall: jest.fn().mockResolvedValue({ result: 'stub' }),
  validateStellarAddress: jest.fn((addr) => typeof addr === 'string' && addr.startsWith('G') && addr.length >= 56),
  NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
  RPC_URL: 'https://soroban-testnet.stellar.org',
}));

// Mock stellar-sdk so the test suite does not require the ESM-only
// uint8array-extras transitive dependency that breaks Jest's CommonJS transform.
jest.mock('@stellar/stellar-sdk', () => {
  // Pre-encoded Ed25519 public keys (Buffer.alloc(32, N) pattern).
  // Generated offline; deterministic — no live network calls.
  const VALID_ADDRESS   = 'GAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';
  const VALID_ADDRESS_2 = 'GABQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';
  const VALID_ADDRESS_3 = 'GACQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';

  return {
    Keypair: {
      fromSecret: jest.fn().mockReturnValue({ publicKey: () => VALID_ADDRESS }),
      random: jest.fn().mockReturnValue({ secret: () => 'SDQFQ4RQMKL7K73TKA3Z6IUXWTUY3XMLHYQH7G4MREIOAG6GZFGIKORJ', publicKey: () => VALID_ADDRESS }),
    },
    nativeToScVal: jest.fn((val) => ({ _mocked: true, value: val })),
    scValToNative: jest.fn().mockReturnValue({}),
    StrKey: {
      encodeEd25519PublicKey: jest.fn((buf) => {
        const idx = buf[0] || 0;
        return ['GAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF',
                'GABQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF',
                'GACQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF'][idx] ||
               'GAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';
      }),
    },
    SorobanRpc: { Server: jest.fn() },
    Networks: { TESTNET: 'Test SDF Network ; September 2015', PUBLIC: 'Public Global Stellar Network ; September 2015' },
    TransactionBuilder: jest.fn(),
    BASE_FEE: '100',
    Contract: jest.fn(),
  };
});

const { simulateContractCall } = require('../services/stellar');
const { nativeToScVal, scValToNative, StrKey } = require('@stellar/stellar-sdk');

// Generate a valid Stellar public key for use in test fixtures.
const VALID_ADDRESS = StrKey.encodeEd25519PublicKey(Buffer.alloc(32, 1));
const VALID_ADDRESS_2 = StrKey.encodeEd25519PublicKey(Buffer.alloc(32, 2));
const VALID_ADDRESS_3 = StrKey.encodeEd25519PublicKey(Buffer.alloc(32, 3));
const INVALID_ADDRESS = 'GABC123';

// Provide a contractId via env so contractIdMiddleware passes.
process.env.VESTING_CONTRACT_ID = 'CTEST0000000000000000000000000000000000000000000000000000';

const app = require('../index');

describe('Vesting routes', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockSimulate.mockResolvedValue({ result: { retval: {} } });
    const { scValToNative } = require('@stellar/stellar-sdk');
    scValToNative.mockReturnValue({});
    // Reset nativeToScVal to return a simple mock object
    const { nativeToScVal: nts } = require('@stellar/stellar-sdk');
    nts.mockReturnValue({ _mocked: true });
  });

  // ── GET /schedule/:id — stub fallback ──────────────────────────────────────
  describe('GET /api/vesting/schedule/:id — stub fallback', () => {
    it('returns schedule stub for a valid id when SOURCE_SECRET_KEY is not set', async () => {
      delete process.env.SOURCE_SECRET_KEY;
      const res = await request(app).get('/api/vesting/schedule/1');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.id).toBe(1);
      expect(res.body.data).toHaveProperty('note');
      expect(simulateContractCall).not.toHaveBeenCalled();
    });
  });

  // ── GET /schedule/:id — live call ──────────────────────────────────────────
  describe('GET /api/vesting/schedule/:id — live call', () => {
    beforeEach(() => {
      process.env.SOURCE_SECRET_KEY = 'SDQFQ4RQMKL7K73TKA3Z6IUXWTUY3XMLHYQH7G4MREIOAG6GZFGIKORJ';
    });
    afterEach(() => { delete process.env.SOURCE_SECRET_KEY; });

    it('calls simulateContractCall with get_schedule and returns parsed schedule', async () => {
      const { scValToNative } = require('@stellar/stellar-sdk');
      scValToNative.mockReturnValue({ beneficiary: 'GABC', totalAmount: '1000' });

      const res = await request(app).get('/api/vesting/schedule/5');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.id).toBe(5);
      expect(res.body.data.beneficiary).toBe('GABC');
      expect(res.body.data.contractId).toBe(process.env.VESTING_CONTRACT_ID);
      expect(simulateContractCall).toHaveBeenCalledTimes(1);
      expect(simulateContractCall).toHaveBeenCalledWith(expect.objectContaining({ method: 'get_schedule' }));
    });

    it('propagates errors from simulateContractCall to the error handler', async () => {
      simulateContractCall.mockRejectedValueOnce(new Error('RPC error'));
      const res = await request(app).get('/api/vesting/schedule/1');
      expect(res.status).toBe(500);
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
      const sourceSecret = 'SDQFQ4RQMKL7K73TKA3Z6IUXWTUY3XMLHYQH7G4MREIOAG6GZFGIKORJ';
      simulateContractCall.mockClear();
      process.env.VESTING_CONTRACT_ID = contractId;
      process.env.SOURCE_SECRET_KEY = sourceSecret;
      simulateContractCall.mockResolvedValueOnce({
        result: { retval: { _mock: 'i128-42000' } },
      });
      scValToNative.mockReturnValueOnce(BigInt(42000));

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

    it('returns 400 when totalDuration is zero', async () => {
      const res = await request(app)
        .post('/api/vesting/schedule')
        .send({ ...valid, totalDuration: 0 });
      expect(res.status).toBe(400);
      expect(res.body.error).toBe('totalDuration must be positive');
    });

    it('returns 400 when totalDuration is missing', async () => {
      const withoutDuration = { ...valid };
      delete withoutDuration.totalDuration;
      const res = await request(app)
        .post('/api/vesting/schedule')
        .send(withoutDuration);
      expect(res.status).toBe(400);
      expect(res.body.error).toBe('totalDuration must be positive');
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

  // ── GET /beneficiary/:addr — stub fallback ────────────────────────────────
  describe('GET /api/vesting/beneficiary/:addr — stub fallback', () => {
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
      expect(res.body.data.scheduleIds).toHaveLength(0);
    });

    it('includes contractId in the response data', async () => {
      const res = await request(app).get(`/api/vesting/beneficiary/${VALID_ADDRESS}`);
      expect(res.status).toBe(200);
      expect(res.body.data).toHaveProperty('contractId');
    });

    it('reflects the address param in the beneficiary field for different addresses', async () => {
      delete process.env.SOURCE_SECRET_KEY;
      const addr = 'GDEF456789012345678901234567890123456789012345678901234567';
      const res = await request(app).get(`/api/vesting/beneficiary/${addr}`);
      expect(res.status).toBe(200);
      expect(res.body.data.beneficiary).toBe(addr);
    });

    it('returns 400 when contractId is missing from env and not supplied', async () => {
      const savedId = process.env.VESTING_CONTRACT_ID;
      delete process.env.VESTING_CONTRACT_ID;
      process.env.VESTING_CONTRACT_ID = savedId;
      const res = await request(app)
        .get(`/api/vesting/beneficiary/${VALID_ADDRESS}`)
        .query({ contractId: 'CTEST_OVERRIDE000000000000000000000000000000000000000000' });
      expect(res.status).toBe(200);
      expect(res.body.data.contractId).toBe('CTEST_OVERRIDE000000000000000000000000000000000000000000');
    });
  });

  // ── GET /beneficiary/:addr — live call ────────────────────────────────────
  describe('GET /api/vesting/beneficiary/:addr — live call', () => {
    // Valid G-address: G + 55 base-32 chars
    const VALID_ADDR = 'GBMOMCDFID6S4ALFZP36ZIVA3RXDEZ6K5IAD5K3IQ3UNYTZHQN4BPMEA';

    beforeEach(() => {
      process.env.SOURCE_SECRET_KEY = 'SDQFQ4RQMKL7K73TKA3Z6IUXWTUY3XMLHYQH7G4MREIOAG6GZFGIKORJ';
    });
    afterEach(() => { delete process.env.SOURCE_SECRET_KEY; });

    it('calls simulateContractCall with get_beneficiary_schedules and returns parsed IDs', async () => {
      const { scValToNative } = require('@stellar/stellar-sdk');
      scValToNative.mockReturnValueOnce([BigInt(1), BigInt(5), BigInt(9)]);

      const res = await request(app).get(`/api/vesting/beneficiary/${VALID_ADDR}`);
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.beneficiary).toBe(VALID_ADDR);
      expect(res.body.data.scheduleIds).toEqual([1, 5, 9]);
      expect(simulateContractCall).toHaveBeenCalledTimes(1);
      expect(simulateContractCall).toHaveBeenCalledWith(
        expect.objectContaining({ method: 'get_beneficiary_schedules' })
      );
    });

    it('returns empty scheduleIds when the contract returns an empty Vec', async () => {
      const { scValToNative } = require('@stellar/stellar-sdk');
      scValToNative.mockReturnValueOnce([]);

      const res = await request(app).get(`/api/vesting/beneficiary/${VALID_ADDR}`);
      expect(res.status).toBe(200);
      expect(res.body.data.scheduleIds).toEqual([]);
    });

    it('returns 400 for an invalid Stellar address', async () => {
      const res = await request(app).get('/api/vesting/beneficiary/not-a-stellar-address');
      expect(res.status).toBe(400);
      expect(res.body).toHaveProperty('error');
      expect(simulateContractCall).not.toHaveBeenCalled();
    });

    it('propagates errors from simulateContractCall to the error handler', async () => {
      simulateContractCall.mockRejectedValueOnce(new Error('RPC error'));
      const res = await request(app).get(`/api/vesting/beneficiary/${VALID_ADDR}`);
      expect(res.status).toBe(500);
    });
  });

  // ── GET /count ─────────────────────────────────────────────────────────────
  describe('GET /api/vesting/count', () => {
    beforeEach(() => { delete process.env.SOURCE_SECRET_KEY; });

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

    it('returns stub count 0 and note field when SOURCE_SECRET_KEY is absent', async () => {
      const res = await request(app).get('/api/vesting/count');
      expect(res.body.data.count).toBe(0);
      expect(res.body.data).toHaveProperty('note');
      expect(simulateContractCall).not.toHaveBeenCalled();
    });
  });

  // ── GET /count — live call ─────────────────────────────────────────────────
  describe('GET /api/vesting/count — live call', () => {
    beforeEach(() => {
      process.env.SOURCE_SECRET_KEY = 'SDQFQ4RQMKL7K73TKA3Z6IUXWTUY3XMLHYQH7G4MREIOAG6GZFGIKORJ';
    });
    afterEach(() => { delete process.env.SOURCE_SECRET_KEY; });

    it('calls simulateContractCall with schedule_count and returns parsed count', async () => {
      const { scValToNative } = require('@stellar/stellar-sdk');
      scValToNative.mockReturnValueOnce(BigInt(42));

      const res = await request(app).get('/api/vesting/count');
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.count).toBe(42);
      expect(res.body.data.contractId).toBe(process.env.VESTING_CONTRACT_ID);
      expect(simulateContractCall).toHaveBeenCalledTimes(1);
      expect(simulateContractCall).toHaveBeenCalledWith(
        expect.objectContaining({ method: 'schedule_count', args: [] })
      );
    });

    it('returns count 0 when the contract returns BigInt(0)', async () => {
      const { scValToNative } = require('@stellar/stellar-sdk');
      scValToNative.mockReturnValueOnce(BigInt(0));

      const res = await request(app).get('/api/vesting/count');
      expect(res.status).toBe(200);
      expect(res.body.data.count).toBe(0);
    });

    it('propagates errors from simulateContractCall to the error handler', async () => {
      simulateContractCall.mockRejectedValueOnce(new Error('RPC error'));
      const res = await request(app).get('/api/vesting/count');
      expect(res.status).toBe(500);
    });
  });
});

'use strict';

const request = require('supertest');
const app = require('../index');

describe('GET /api/health', () => {
  it('returns status ok with timestamp and network', async () => {
    const res = await request(app).get('/api/health');
    expect(res.status).toBe(200);
    expect(res.body.status).toBe('ok');
    expect(res.body.timestamp).toBeDefined();
    expect(res.body.network).toBeDefined();
    expect(res.body.rpc).toBeDefined();
  });
});

describe('GET /api/health/ready', () => {
  const ENV_KEYS = ['VESTING_CONTRACT_ID', 'TOKEN_CONTRACT_ID', 'MULTISIG_CONTRACT_ID'];

  afterEach(() => {
    ENV_KEYS.forEach((k) => delete process.env[k]);
  });

  it('returns 503 with ready:false when no contract IDs are set', async () => {
    const res = await request(app).get('/api/health/ready');
    expect(res.status).toBe(503);
    expect(res.body.ready).toBe(false);
    expect(res.body.contracts.vesting).toBe(false);
    expect(res.body.contracts.token).toBe(false);
    expect(res.body.contracts.multisig).toBe(false);
  });

  it('returns 200 with ready:true when at least one contract ID is set', async () => {
    process.env.VESTING_CONTRACT_ID = 'CTEST';
    const res = await request(app).get('/api/health/ready');
    expect(res.status).toBe(200);
    expect(res.body.ready).toBe(true);
    expect(res.body.contracts.vesting).toBe(true);
    expect(res.body.contracts.token).toBe(false);
    expect(res.body.contracts.multisig).toBe(false);
  });

  it('returns 200 with all true when all contract IDs are set', async () => {
    process.env.VESTING_CONTRACT_ID = 'CV';
    process.env.TOKEN_CONTRACT_ID = 'CT';
    process.env.MULTISIG_CONTRACT_ID = 'CM';
    const res = await request(app).get('/api/health/ready');
    expect(res.status).toBe(200);
    expect(res.body.ready).toBe(true);
    expect(res.body.contracts.vesting).toBe(true);
    expect(res.body.contracts.token).toBe(true);
    expect(res.body.contracts.multisig).toBe(true);
  });

  it('response includes per-contract boolean status', async () => {
    process.env.TOKEN_CONTRACT_ID = 'CT';
    const res = await request(app).get('/api/health/ready');
    expect(res.status).toBe(200);
    expect(res.body.contracts).toHaveProperty('vesting');
    expect(res.body.contracts).toHaveProperty('token');
    expect(res.body.contracts).toHaveProperty('multisig');
  });
});

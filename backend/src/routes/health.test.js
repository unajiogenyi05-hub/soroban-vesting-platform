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

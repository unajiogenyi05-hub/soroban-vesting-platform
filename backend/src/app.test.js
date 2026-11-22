'use strict';

/**
 * App-level integration tests (routes, middleware, fallback handlers).
 *
 * Covers the 404 catch-all handler in src/index.js, which returns
 * { error: 'Not found' } for any unmatched route.
 */

const request = require('supertest');
const app = require('./index');

describe('Unknown routes', () => {
  it('returns 404 JSON for unknown routes', async () => {
    const res = await request(app).get('/api/does-not-exist');
    expect(res.status).toBe(404);
    expect(res.body.error).toBeDefined();
  });

  it('returns 404 for unknown non-/api routes', async () => {
    const res = await request(app).get('/nope');
    expect(res.status).toBe(404);
    expect(res.body.error).toBe('Not found');
  });

  it('returns 404 for unknown POST routes', async () => {
    const res = await request(app).post('/api/does-not-exist');
    expect(res.status).toBe(404);
    expect(res.body.error).toBeDefined();
  });
});

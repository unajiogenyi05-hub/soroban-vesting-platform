'use strict';

const contractIdMiddleware = require('./contractId');

describe('contractIdMiddleware', () => {
  let req;
  let res;
  let next;

  beforeEach(() => {
    req = { body: {}, query: {} };
    res = {
      status: jest.fn().mockReturnThis(),
      json: jest.fn(),
    };
    next = jest.fn();
  });

  it('uses contractId from the request body', () => {
    const mw = contractIdMiddleware('CENV0000000000000000000000000000000000000000000000000000');
    req.body.contractId = 'CBODY0000000000000000000000000000000000000000000000000000';
    mw(req, res, next);
    expect(req.contractId).toBe('CBODY0000000000000000000000000000000000000000000000000000');
    expect(next).toHaveBeenCalledTimes(1);
    expect(res.json).not.toHaveBeenCalled();
  });

  it('falls back to contractId from the query string', () => {
    const mw = contractIdMiddleware('CENV0000000000000000000000000000000000000000000000000000');
    req.query.contractId = 'CQUERY0000000000000000000000000000000000000000000000000000';
    mw(req, res, next);
    expect(req.contractId).toBe('CQUERY0000000000000000000000000000000000000000000000000000');
    expect(next).toHaveBeenCalledTimes(1);
  });

  it('falls back to the env contract id when body and query are absent', () => {
    const mw = contractIdMiddleware('CENV0000000000000000000000000000000000000000000000000000');
    mw(req, res, next);
    expect(req.contractId).toBe('CENV0000000000000000000000000000000000000000000000000000');
    expect(next).toHaveBeenCalledTimes(1);
  });

  it('returns 400 when no contract id can be resolved', () => {
    const mw = contractIdMiddleware('');
    mw(req, res, next);
    expect(res.status).toHaveBeenCalledWith(400);
    expect(res.json).toHaveBeenCalledWith({ error: 'contractId is required' });
    expect(next).not.toHaveBeenCalled();
  });

  it('gives body precedence over query and env fallback', () => {
    const mw = contractIdMiddleware('CENV0000000000000000000000000000000000000000000000000000');
    req.body.contractId = 'CBODY0000000000000000000000000000000000000000000000000000';
    req.query.contractId = 'CQUERY0000000000000000000000000000000000000000000000000000';
    mw(req, res, next);
    expect(req.contractId).toBe('CBODY0000000000000000000000000000000000000000000000000000');
    expect(next).toHaveBeenCalledTimes(1);
  });
});

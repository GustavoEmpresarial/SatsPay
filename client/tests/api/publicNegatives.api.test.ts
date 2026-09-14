/**
 * Live API negatives + availability (#5, #29).
 * Default target: public prod. Override with SATSPAY_API_BASE.
 * Skip with SATSPAY_API_LIVE=0.
 */
import { describe, expect, it } from 'vitest';
import { assertSwapPricesResponse } from '../helpers/contracts.js';

const enabled = process.env.SATSPAY_API_LIVE !== '0';
const base = (process.env.SATSPAY_API_BASE || 'https://www.satspay.pro').replace(/\/$/, '');

async function req(path: string, init?: RequestInit): Promise<Response> {
  return fetch(`${base}${path}`, {
    ...init,
    headers: {
      Accept: 'application/json',
      ...(init?.headers ?? {}),
    },
  });
}

describe.skipIf(!enabled)('API live: availability + negatives', () => {
  it('GET /healthz is alive', async () => {
    const res = await req('/healthz');
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text.toLowerCase()).toMatch(/ok|healthy|alive|true/i);
  }, 20_000);

  it('GET /v1/auth/me without token → 401', async () => {
    const res = await req('/v1/auth/me');
    expect([401, 403]).toContain(res.status);
  }, 20_000);

  it('POST /v1/auth/login empty body → 4xx', async () => {
    const res = await req('/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
    });
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.status).toBeLessThan(500);
  }, 20_000);

  it('POST /v1/auth/login malformed JSON → 4xx', async () => {
    const res = await req('/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{not-json',
    });
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.status).toBeLessThan(500);
  }, 20_000);

  it('GET /v1/swap/prices returns contract-shaped payload', async () => {
    const res = await req('/v1/swap/prices');
    expect(res.status).toBe(200);
    const json = await res.json();
    expect(assertSwapPricesResponse(json)).toEqual([]);
  }, 20_000);

  it('admin route without auth → 401/403', async () => {
    const res = await req('/v1/admin/stats');
    expect([401, 403, 404]).toContain(res.status);
  }, 20_000);
});

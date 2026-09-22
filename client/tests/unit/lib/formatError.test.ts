import { describe, expect, it } from 'vitest';
import { ApiError } from '../../../src/lib/api.js';
import { formatApiError } from '../../../src/lib/formatError.js';

describe('formatApiError', () => {
  it('translates captcha verification failure', () => {
    const err = new ApiError(400, 'ERROR', 'captcha verification failed');
    expect(formatApiError(err)).toContain('anti-bot');
  });

  it('translates unauthorized / refresh reuse', () => {
    expect(formatApiError(new ApiError(401, 'ERROR', 'unauthorized'))).toMatch(/Sessão|atualize|entre/i);
    expect(formatApiError(new ApiError(401, 'ERROR', 'refresh reuse detected'))).toMatch(/segurança|Entre/i);
  });

  it('uses 401 fallback when message is opaque', () => {
    expect(formatApiError(new ApiError(401, 'ERROR', 'nope'))).toMatch(/Sessão expirada/i);
  });

  it('uses 415 fallback for unsupported media type', () => {
    expect(formatApiError(new ApiError(415, 'ERROR', 'API error 415'))).toMatch(/Content-Type/i);
  });

  it('flattens VALIDATION_ERROR fieldErrors', () => {
    const err = new ApiError(400, 'VALIDATION_ERROR', 'bad', {
      fieldErrors: { email: ['required'], password: ['too short'] },
      formErrors: ['form broken'],
    });
    const msg = formatApiError(err);
    expect(msg).toContain('email: required');
    expect(msg).toContain('password: too short');
    expect(msg).toContain('form broken');
  });

  it('handles empty validation details', () => {
    const err = new ApiError(400, 'VALIDATION_ERROR', 'opaque', {
      fieldErrors: { email: undefined as unknown as string[], nick: null as unknown as string[] },
    });
    expect(formatApiError(err)).toBe('opaque');
  });

  it('passes through plain Error when no translation matches', () => {
    expect(formatApiError(new Error('wallet exploded'))).toBe('wallet exploded');
  });

  it('uses fallback for non-Error values and empty ApiError message', () => {
    expect(formatApiError(null, 'custom-fallback')).toBe('custom-fallback');
    expect(formatApiError(new ApiError(500, 'ERROR', ''), 'fb')).toBe('fb');
  });

  it('translates plain Error messages via dictionary', () => {
    expect(formatApiError(new Error('insufficient balance'))).toMatch(/Saldo insuficiente/i);
    expect(
      formatApiError(new ApiError(400, 'ERROR', 'platform inventory for Btc is insufficient for this operation')),
    ).toMatch(/Inventário da plataforma/i);
  });

  it.each([
    ['BNB_GAS_REQUIRED', /BNB/],
    ['WITHDRAWAL_MERCHANT_BLOCKED', /caixa de comerciante/],
    ['SLIPPAGE', /nova cotação/],
    ['AMOUNT_TOO_LOW', /Valor baixo demais/],
  ])('maps stable code %s regardless of the raw message', (code, expected) => {
    expect(formatApiError(new ApiError(400, code, '{"raw":"provider json"}'))).toMatch(expected);
  });

  it('translates plain Errors and passes unknown ones through', () => {
    expect(formatApiError(new Error('Insufficient balance on wallet'))).toMatch(/Saldo insuficiente/);
    expect(formatApiError(new Error('boom'))).toBe('boom');
  });

  it('falls back for non-Error values and empty messages', () => {
    expect(formatApiError('nope', 'fb')).toBe('fb');
    expect(formatApiError(new ApiError(500, 'ERROR', ''), 'fb')).toBe('fb');
  });
});

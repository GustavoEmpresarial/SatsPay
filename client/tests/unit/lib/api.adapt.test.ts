import { describe, expect, it } from 'vitest';
import { adaptRustError, adaptRustResponse, looksLikeJsonBody } from '../../../src/lib/api.js';

describe('adaptRustResponse', () => {
  it('flattens login tokens envelope without refreshToken', () => {
    const out = adaptRustResponse('/auth/login', {
      user: { id: '1', email: 'a@b.c', two_factor_enabled: true, merchant_status: 'NONE' },
      tokens: { accessToken: 'a' },
    }) as { accessToken: string; refreshToken?: string; user: { twoFactorEnabled: boolean; username: string } };
    expect(out.accessToken).toBe('a');
    expect(out.refreshToken).toBeUndefined();
    expect(out.user.twoFactorEnabled).toBe(true);
    expect(out.user.username).toBe('');
  });

  it('maps code_sent challenge', () => {
    const out = adaptRustResponse('/auth/login', { kind: 'code_sent', email: 'x@y.z' }) as {
      codeSent: boolean;
      message: string;
    };
    expect(out.codeSent).toBe(true);
    expect(out.message).toContain('x@y.z');
  });

  it('normalizes refresh access token only', () => {
    const out = adaptRustResponse('/auth/refresh', {
      access_token: 'aa',
    }) as { accessToken?: string; refreshToken?: string };
    expect(out.accessToken).toBe('aa');
    expect(out.refreshToken).toBeUndefined();
  });

  it('wraps wallet array', () => {
    const out = adaptRustResponse('/wallet?kind=PERSONAL', [{ id: 1 }]) as { wallets: unknown[] };
    expect(out.wallets).toHaveLength(1);
  });

  it('passes through unrelated payloads', () => {
    expect(adaptRustResponse('/swap/prices', { BTC: 1 })).toEqual({ BTC: 1 });
    expect(adaptRustResponse('/x', null)).toBeNull();
  });

  it('normalizes /auth/me and register envelopes', () => {
    const me = adaptRustResponse('/auth/me', {
      user: { id: '1', two_factor_enabled: true, merchant_status: 'APPROVED', userName: 'alice' },
    }) as { user: { username: string; twoFactorEnabled: boolean; merchantStatus: string } };
    expect(me.user.username).toBe('alice');
    expect(me.user.twoFactorEnabled).toBe(true);
    expect(me.user.merchantStatus).toBe('APPROVED');

    const reg = adaptRustResponse('/auth/register', {
      user: { id: '2', username: 'bob' },
      tokens: { accessToken: 'tok' },
    }) as { accessToken: string; user: { username: string } };
    expect(reg.accessToken).toBe('tok');
    expect(reg.user.username).toBe('bob');
  });
});

describe('looksLikeJsonBody', () => {
  it('detects object and array literals', () => {
    expect(looksLikeJsonBody('  {"a":1}')).toBe(true);
    expect(looksLikeJsonBody('[1]')).toBe(true);
    expect(looksLikeJsonBody('plain')).toBe(false);
    expect(looksLikeJsonBody('')).toBe(false);
  });
});

describe('adaptRustError', () => {
  it('parses string and structured error envelopes', () => {
    expect(adaptRustError({ error: 'unauthorized' }, 'x')).toEqual({
      code: 'ERROR',
      message: 'unauthorized',
    });
    expect(adaptRustError({ error: { code: 'CAPTCHA', message: 'nope', details: 1 } }, 'x')).toEqual({
      code: 'CAPTCHA',
      message: 'nope',
      details: 1,
    });
    expect(adaptRustError({}, 'Bad Gateway')).toEqual({ code: 'UNKNOWN', message: 'Bad Gateway' });
  });

  it('reads the flat { error, code } shape most Rust handlers return', () => {
    expect(adaptRustError({ error: 'cannot send to yourself', code: 'SEND_TO_SELF' }, 'x')).toEqual({
      code: 'SEND_TO_SELF',
      message: 'cannot send to yourself',
      details: undefined,
    });
    expect(
      adaptRustError({ error: 'paused', code: 'WITHDRAWAL_MERCHANT_BLOCKED', coin: 'BTC' }, 'x'),
    ).toEqual({ code: 'WITHDRAWAL_MERCHANT_BLOCKED', message: 'paused', details: { coin: 'BTC' } });
    // Non-string / empty code falls back to the legacy generic code.
    expect(adaptRustError({ error: 'x', code: 42 }, 'y').code).toBe('ERROR');
    expect(adaptRustError({ error: 'x', code: '' }, 'y').code).toBe('ERROR');
  });

  it('maps legacy faucet cooldown string to FAUCET_COOLDOWN', () => {
    const out = adaptRustError(
      { error: 'next claim available at 2026-09-10 16:20:20.474071 UTC' },
      'Bad Request',
    );
    expect(out.code).toBe('FAUCET_COOLDOWN');
    expect(out.message).toBe('Aguarde o cooldown de 11 horas do faucet.');
    expect((out.details as { nextClaimAt?: string })?.nextClaimAt).toContain('2026-09-10');
  });
});

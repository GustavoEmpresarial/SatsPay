import { describe, expect, it } from 'vitest';
import { classifyError, isExpectedAuthNoise, redactSecrets } from '../../../src/lib/errorConsole.js';

describe('errorConsole', () => {
  it('redacts bearer, jwt, api keys and hex secrets', () => {
    const raw =
      'Bearer tokensecretvalue99 eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.aaaaaaaa.bbbbbbbb sk_live_abcdefghijklmnopqrst password=supersecret 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
    const out = redactSecrets(raw);
    expect(out).toContain('[REDACTED]');
    expect(out).not.toContain('tokensecretvalue99');
    expect(out).not.toContain('sk_live_abcdefghijklmnopqrst');
    expect(out).not.toContain('supersecret');
    expect(out).not.toContain('0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef');
    expect(out).toContain('[REDACTED_JWT]');
  });

  it('classifies auth / business / client like TubeCoin console', () => {
    const auth = classifyError({
      id: 'err_68abcdefd2a8',
      fingerprint: 'fp1',
      service: 'api-server',
      level: 'WARN',
      message: 'Not logged in.',
      status_code: 401,
      occurrences_count: 26,
      status: 'OPEN',
      first_seen_at: new Date().toISOString(),
      last_seen_at: new Date().toISOString(),
    });
    expect(auth.code).toBe('AUTH_NOT_LOGGED_IN');
    expect(auth.category).toBe('AUTH');
    expect(auth.lifecycle).toBe('NEW');
    expect(auth.origin).toBe('api');

    const biz = classifyError({
      id: 'err_bb6021xxxx',
      fingerprint: 'fp2',
      service: 'api-server',
      level: 'WARN',
      message: 'Not enough Zems for a ticket.',
      status_code: 400,
      occurrences_count: 1,
      status: 'OPEN',
      first_seen_at: new Date().toISOString(),
      last_seen_at: new Date().toISOString(),
    });
    expect(biz.code).toBe('INSUFFICIENT_ZEMS');
    expect(biz.category).toBe('BUSINESS');

    const client = classifyError({
      id: 'err_cd6380xxxx',
      fingerprint: 'fp3',
      service: 'client-frontend',
      level: 'WARN',
      message: 'The provider is disconnected from all chains.',
      endpoint: 'http://localhost/',
      occurrences_count: 12,
      status: 'OPEN',
      first_seen_at: new Date().toISOString(),
      last_seen_at: new Date().toISOString(),
    });
    expect(client.code).toBe('CLIENT_JS_ERROR');
    expect(client.category).toBe('CLIENT');
    expect(client.origin).toBe('client');
  });

  it('flags expected auth noise', () => {
    expect(isExpectedAuthNoise({ message: 'Not logged in.', status_code: 401 })).toBe(true);
    expect(isExpectedAuthNoise({ message: 'boom', status_code: 500 })).toBe(false);
  });
});

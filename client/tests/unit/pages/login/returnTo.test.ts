import { describe, expect, it } from 'vitest';
import { resolveReturnTo } from '../../../../src/lib/returnTo.js';

describe('resolveReturnTo (login open-redirect guard)', () => {
  it('uses fallback when missing', () => {
    expect(resolveReturnTo(null)).toBe('/dashboard');
    expect(resolveReturnTo(undefined)).toBe('/dashboard');
    expect(resolveReturnTo('')).toBe('/dashboard');
  });

  it('allows safe relative paths', () => {
    expect(resolveReturnTo('/wallets')).toBe('/wallets');
    expect(resolveReturnTo('/deposit?x=1')).toBe('/deposit?x=1');
    expect(resolveReturnTo('%2Fsettings')).toBe('/settings');
  });

  it('blocks absolute and protocol-relative URLs', () => {
    expect(resolveReturnTo('https://evil.example')).toBe('/dashboard');
    expect(resolveReturnTo('//evil.example')).toBe('/dashboard');
    expect(resolveReturnTo('/\\evil')).toBe('/dashboard');
    expect(resolveReturnTo('javascript:alert(1)')).toBe('/dashboard');
  });

  it('keeps raw string when decodeURIComponent throws', () => {
    // lone % is invalid URI sequence
    expect(resolveReturnTo('/%')).toBe('/%');
  });
});

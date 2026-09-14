/**
 * Business acceptance / rules (#15, #68, #69) — executable acceptance criteria
 * for money-critical paths (client-side gates that mirror product rules).
 */
import { describe, expect, it } from 'vitest';
import { COIN_CONFIG, computeSwap, SWAP_DEFAULT_FEE_BPS } from '../../../src/shared/coins.js';
import { canWithdraw, isPlausibleAddress, parseHumanAmount } from '../../../src/lib/amountInput.js';
import { passwordIssues, passwordsMatch, usernameIssue } from '../../../src/lib/authValidation.js';
import { resolveReturnTo } from '../../../src/lib/returnTo.js';

describe('acceptance: registration rules', () => {
  it('user cannot register with weak password or bad username', () => {
    expect(passwordIssues('12345678').length).toBeGreaterThan(0);
    expect(usernameIssue('ab')).toBe('usernameLength');
    expect(passwordsMatch('GoodPass1x', 'GoodPass1y')).toBe(false);
  });
});

describe('acceptance: withdrawal rules', () => {
  it('user cannot withdraw more than balance including fee', () => {
    const fee = COIN_CONFIG.BTC.withdrawalFee;
    const bal = 50_000n;
    const amount = bal; // would exceed with fee
    expect(canWithdraw({ amount, balance: bal, fee, minWithdrawal: 1n }).ok).toBe(false);
  });

  it('user cannot withdraw to implausible address', () => {
    expect(isPlausibleAddress('xx')).toBe(false);
    expect(isPlausibleAddress('bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh')).toBe(true);
  });

  it('zero / empty amount does not unlock submit', () => {
    expect(parseHumanAmount('', 'BTC')).toBe(0n);
    expect(canWithdraw({ amount: 0n, balance: 1_000_000n, fee: 1n, minWithdrawal: 1n }).ok).toBe(
      false,
    );
  });
});

describe('acceptance: swap fee transparency', () => {
  it('default fee bps is applied and disclosed in quote', () => {
    const q = computeSwap('BTC', 'LTC', 100_000_000n, 100n, 1n, SWAP_DEFAULT_FEE_BPS);
    expect(q.feeBps).toBe(SWAP_DEFAULT_FEE_BPS);
    expect(q.feeAmount).toBeGreaterThan(0n);
  });
});

describe('acceptance: open-redirect guard after login', () => {
  it('external returnTo never wins over dashboard fallback', () => {
    expect(resolveReturnTo('https://evil.test')).toBe('/dashboard');
    expect(resolveReturnTo('//evil.test')).toBe('/dashboard');
    expect(resolveReturnTo('/wallets')).toBe('/wallets');
  });
});

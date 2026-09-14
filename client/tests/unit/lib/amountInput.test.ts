import { describe, expect, it } from 'vitest';
import {
  canWithdraw,
  isPlausibleAddress,
  netAfterWithdrawalFee,
  parseHumanAmount,
} from '../../../src/lib/amountInput.js';

describe('parseHumanAmount', () => {
  it('parses decimals into 8-place ledger units', () => {
    expect(parseHumanAmount('1', 'BTC')).toBe(100_000_000n);
    expect(parseHumanAmount('0.00000001', 'BTC')).toBe(1n);
    expect(parseHumanAmount('1.5', 'LTC')).toBe(150_000_000n);
  });

  it('returns 0 for invalid', () => {
    expect(parseHumanAmount('', 'BTC')).toBe(0n);
    expect(parseHumanAmount('abc', 'BTC')).toBe(0n);
    expect(parseHumanAmount('-1', 'BTC')).toBe(0n);
    expect(parseHumanAmount('0', 'BTC')).toBe(0n);
    expect(parseHumanAmount('  1.25  ', 'BTC')).toBe(125_000_000n);
  });

  it('falls back to 8 decimals for unknown coin cast', () => {
    expect(parseHumanAmount('1', 'ETH' as import('../../../src/shared/coins.js').Coin)).toBe(100_000_000n);
  });
});

describe('netAfterWithdrawalFee', () => {
  it('subtracts fee and floors at zero', () => {
    expect(netAfterWithdrawalFee(1000n, 100n)).toBe(900n);
    expect(netAfterWithdrawalFee(50n, 100n)).toBe(0n);
  });
});

describe('canWithdraw', () => {
  it('validates zero / min / balance', () => {
    expect(canWithdraw({ amount: 0n, balance: 1_000n, fee: 10n, minWithdrawal: 1n }).reason).toBe('zero');
    expect(canWithdraw({ amount: 5n, balance: 1_000n, fee: 10n, minWithdrawal: 10n }).reason).toBe('belowMin');
    expect(canWithdraw({ amount: 100n, balance: 100n, fee: 10n, minWithdrawal: 1n }).reason).toBe(
      'insufficient',
    );
    expect(canWithdraw({ amount: 100n, balance: 200n, fee: 10n, minWithdrawal: 1n }).ok).toBe(true);
  });
});

describe('isPlausibleAddress', () => {
  it('rejects short / spaced / empty', () => {
    expect(isPlausibleAddress('')).toBe(false);
    expect(isPlausibleAddress('abc')).toBe(false);
    expect(isPlausibleAddress('bc1q with spaces here!!')).toBe(false);
  });

  it('accepts typical lengths', () => {
    expect(isPlausibleAddress('bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh')).toBe(true);
    expect(isPlausibleAddress('x'.repeat(129))).toBe(false);
  });
});

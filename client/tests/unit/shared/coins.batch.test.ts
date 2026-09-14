import { describe, expect, it } from 'vitest';
import {
  COIN_CONFIG,
  COINS,
  formatAmount,
  parseAmount,
} from '../../../src/shared/coins.js';

describe('coin batch coverage', () => {
  it('defines faucet rewards and fees for every coin', () => {
    for (const coin of COINS) {
      const cfg = COIN_CONFIG[coin];
      expect(cfg.symbol).toBe(coin);
      expect(cfg.faucetReward).toBeGreaterThan(0n);
      expect(cfg.minWithdrawal).toBeGreaterThan(0n);
      expect(cfg.withdrawalFee).toBeGreaterThanOrEqual(0n);
      expect(cfg.approvalThreshold).toBeGreaterThan(cfg.minWithdrawal);
    }
  });

  it('formats dust amounts without scientific notation', () => {
    expect(formatAmount(1n, 'BTC')).toBe('0.00000001');
    expect(formatAmount(0n, 'DOGE')).toBe('0');
  });

  it('treats empty and non-numeric parseAmount as zero', () => {
    expect(parseAmount('', 'BTC')).toBe(0n);
    expect(parseAmount('abc', 'BTC')).toBe(0n);
  });

  it('parses all tickers for one unit', () => {
    for (const coin of COINS) {
      expect(parseAmount('1', coin)).toBe(10n ** 8n);
    }
  });
});

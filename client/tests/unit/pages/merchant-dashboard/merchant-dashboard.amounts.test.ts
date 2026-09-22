import { describe, expect, it } from 'vitest';
import { formatLedgerAmount, getCoinUsdValue } from '@/shared';

describe('merchant dashboard invoice amounts', () => {
  it('VIP 7.20 USD BCH invoice is not millions of dollars', () => {
    // Prod sample: amount=3305179 ledger units, price_usd_scaled=720000000 @ 8dp
    const usd = Number('720000000') / 10 ** 8;
    expect(usd).toBeCloseTo(7.2, 5);
    expect(formatLedgerAmount('3305179', 'BCH')).toBe('0.03305179');
  });

  it('raw amount times price without scale is the millionaire bug', () => {
    const buggy = Number('3305179') * 220;
    expect(buggy).toBeGreaterThan(700_000_000);
    const fixed = getCoinUsdValue('3305179', 'BCH', { BCH: String(220 * 1e8) }, 8);
    expect(fixed).toBeLessThan(20);
  });
});

import { describe, expect, it } from 'vitest';
import { aggregateByType, analyticsBucket } from '../../../src/lib/analyticsLedger.js';

describe('analyticsLedger', () => {
  it('drops ADJUSTMENT from buckets', () => {
    expect(analyticsBucket('ADJUSTMENT')).toBeNull();
  });

  it('nets withdrawal reversals against saque volume', () => {
    const rows = aggregateByType([
      { type: 'WITHDRAWAL', amount: '-100', absUsd: 10 },
      { type: 'WITHDRAWAL', amount: '-100', absUsd: 10 },
      { type: 'WITHDRAWAL_REVERSAL', amount: '100', absUsd: 10 },
      { type: 'ADJUSTMENT', amount: '999999', absUsd: 500 },
      { type: 'FAUCET', amount: '1', absUsd: 0.001 },
      { type: 'MERCHANT_CHECKOUT', amount: '-50', absUsd: 17.43 },
    ]);
    const byType = Object.fromEntries(rows.map((r) => [r.type, r]));
    expect(byType.ADJUSTMENT).toBeUndefined();
    expect(byType.WITHDRAWAL?.usd).toBeCloseTo(10, 5);
    expect(byType.WITHDRAWAL?.count).toBe(3);
    expect(byType.MERCHANT_CHECKOUT?.usd).toBeCloseTo(17.43, 5);
    expect(byType.FAUCET?.usd).toBeCloseTo(0.001, 5);
  });

  it('folds merchant deposit into depósito bucket', () => {
    const rows = aggregateByType([
      { type: 'DEPOSIT', amount: '1', absUsd: 5 },
      { type: 'MERCHANT_DEPOSIT', amount: '1', absUsd: 4 },
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0]!.type).toBe('DEPOSIT');
    expect(rows[0]!.usd).toBe(9);
  });
});

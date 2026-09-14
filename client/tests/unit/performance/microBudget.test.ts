import { describe, expect, it } from 'vitest';
import { formatAmount, parseAmount, safeBigInt, computeSwap } from '../../../src/shared/coins.js';

/** Lightweight performance / regression budget (#9) — not a load test. */
describe('performance budgets (micro)', () => {
  it('formatAmount 20k calls stays under 250ms', () => {
    const start = performance.now();
    for (let i = 0; i < 20_000; i += 1) {
      formatAmount(BigInt(i), 'BTC');
    }
    expect(performance.now() - start).toBeLessThan(250);
  });

  it('parseAmount + safeBigInt 10k stays under 250ms', () => {
    const start = performance.now();
    for (let i = 0; i < 10_000; i += 1) {
      parseAmount(`1.${String(i % 100).padStart(2, '0')}`, 'LTC');
      safeBigInt(String(i * 13));
    }
    expect(performance.now() - start).toBeLessThan(250);
  });

  it('computeSwap 2k quotes stays under 250ms', () => {
    const start = performance.now();
    for (let i = 1; i <= 2_000; i += 1) {
      computeSwap('BTC', 'USDT', BigInt(i) * 1000n, 80_000_0000_0000n, 1_0000_0000n, 25);
    }
    expect(performance.now() - start).toBeLessThan(250);
  });
});

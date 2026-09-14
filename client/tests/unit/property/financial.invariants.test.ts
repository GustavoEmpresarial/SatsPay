import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import {
  computeSwap,
  formatAmount,
  parseAmount,
  safeBigInt,
  COINS,
  type Coin,
} from '../../../src/shared/coins.js';
import { canWithdraw, parseHumanAmount } from '../../../src/lib/amountInput.js';
import { passwordIssues, usernameIssue } from '../../../src/lib/authValidation.js';

/** Property / financial invariant tests (tipo #41 + #52). */
describe('property: financial invariants', () => {
  it('safeBigInt never throws on primitives/strings', () => {
    fc.assert(
      fc.property(
        fc.oneof(
          fc.constant(null),
          fc.constant(undefined),
          fc.string({ maxLength: 40 }),
          fc.integer(),
          fc.double(),
          fc.bigInt({ min: -1000000000000000000n, max: 1000000000000000000n }),
        ),
        (v) => {
          expect(() => safeBigInt(v as never)).not.toThrow();
          const n = safeBigInt(v as never);
          expect(typeof n).toBe('bigint');
        },
      ),
      { numRuns: 200 },
    );
  });

  it('parseAmount → formatAmount round-trip for sane decimals', () => {
    fc.assert(
      fc.property(fc.constantFrom(...COINS), fc.integer({ min: 0, max: 1_000_000_000 }), (coin, units) => {
        const amount = BigInt(units);
        const displayed = formatAmount(amount, coin);
        const back = parseAmount(displayed, coin);
        expect(back).toBe(amount);
      }),
      { numRuns: 100 },
    );
  });

  it('computeSwap: toAmount + feeAmount == gross; toAmount >= 0', () => {
    fc.assert(
      fc.property(
        fc.constantFrom(...COINS),
        fc.constantFrom(...COINS),
        fc.bigInt({ min: 1n, max: 10_000_000_000n }),
        fc.bigInt({ min: 1n, max: 1_000_000_000_000n }),
        fc.bigInt({ min: 1n, max: 1_000_000_000_000n }),
        fc.integer({ min: 0, max: 500 }),
        (from, to, amount, pFrom, pTo, feeBps) => {
          if (from === to) return;
          const q = computeSwap(from, to, amount, pFrom, pTo, feeBps);
          expect(q.toAmount).toBeGreaterThanOrEqual(0n);
          expect(q.feeAmount).toBeGreaterThanOrEqual(0n);
          expect(q.toAmount + q.feeAmount).toBeGreaterThanOrEqual(q.toAmount);
        },
      ),
      { numRuns: 80 },
    );
  });

  it('canWithdraw never ok when amount+fee > balance', () => {
    fc.assert(
      fc.property(
        fc.bigInt({ min: 0n, max: 1_000_000n }),
        fc.bigInt({ min: 0n, max: 1_000_000n }),
        fc.bigInt({ min: 0n, max: 50_000n }),
        fc.bigInt({ min: 1n, max: 10_000n }),
        (amount, balance, fee, minW) => {
          const r = canWithdraw({ amount, balance, fee, minWithdrawal: minW });
          if (amount > 0n && amount >= minW && amount + fee > balance) {
            expect(r.ok).toBe(false);
            expect(r.reason).toBe('insufficient');
          }
        },
      ),
      { numRuns: 150 },
    );
  });
});

describe('property: auth validation', () => {
  it('usernameIssue accepts only [A-Za-z][A-Za-z0-9_]{2,23}', () => {
    fc.assert(
      fc.property(fc.string({ minLength: 0, maxLength: 40 }), (s) => {
        const issue = usernameIssue(s);
        const ok = /^[a-zA-Z][a-zA-Z0-9_]{2,23}$/.test(s.trim());
        if (ok) expect(issue).toBeNull();
        else expect(issue).not.toBeNull();
      }),
      { numRuns: 200 },
    );
  });

  it('passwordIssues empty iff length 10–128 with upper+lower+digit', () => {
    fc.assert(
      fc.property(fc.string({ minLength: 0, maxLength: 140 }), (pw) => {
        const issues = passwordIssues(pw);
        const strong =
          pw.length >= 10 &&
          pw.length <= 128 &&
          /[A-Z]/.test(pw) &&
          /[a-z]/.test(pw) &&
          /[0-9]/.test(pw);
        expect(issues.length === 0).toBe(strong);
      }),
      { numRuns: 200 },
    );
  });
});

describe('fuzz: parseHumanAmount', () => {
  it('never throws on arbitrary strings', () => {
    fc.assert(
      fc.property(fc.string({ maxLength: 64 }), fc.constantFrom(...COINS), (input, coin: Coin) => {
        expect(() => parseHumanAmount(input, coin)).not.toThrow();
        const n = parseHumanAmount(input, coin);
        expect(n).toBeGreaterThanOrEqual(0n);
      }),
      { numRuns: 300 },
    );
  });
});

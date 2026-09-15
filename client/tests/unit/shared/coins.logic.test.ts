import { describe, expect, it } from 'vitest';
import {
  computeSwap,
  formatAmount,
  formatUsdValue,
  getCoinUsdValue,
  isCoin,
  isDepositWithdrawPaused,
  isSwapL2Coin,
  isSwapL2Pair,
  defaultDepositWithdrawCoin,
  parseAmount,
  safeBigInt,
  SWAP_DEFAULT_FEE_BPS,
  SWAP_L2_COINS,
  DEPOSIT_WITHDRAW_PAUSED_COINS,
  COINS,
  COIN_CONFIG,
  FALLBACK_PRICES,
  formatLedgerAmount,
  type Coin,
} from '../../../src/shared/coins.js';

describe('isCoin / COINS', () => {
  it('accepts all configured symbols', () => {
    for (const c of COINS) expect(isCoin(c)).toBe(true);
    expect(isCoin('ETH')).toBe(false);
    expect(isCoin('')).toBe(false);
  });
});

describe('swap L2 allowlist', () => {
  it('allows only POL/USDT/USDC pairs', () => {
    expect(isSwapL2Coin('POL')).toBe(true);
    expect(isSwapL2Coin('BTC')).toBe(false);
    expect(isSwapL2Pair('POL', 'USDT')).toBe(true);
    expect(isSwapL2Pair('POL', 'POL')).toBe(false);
    expect(isSwapL2Pair('BTC', 'LTC')).toBe(false);
    expect(SWAP_L2_COINS).toEqual(['POL', 'USDT', 'USDC']);
  });
});

describe('deposit/withdraw pause list', () => {
  it('pauses BTC LTC DOGE but keeps them as coins', () => {
    expect(DEPOSIT_WITHDRAW_PAUSED_COINS).toEqual(['BTC', 'LTC', 'DOGE', 'DGB']);
    expect(isDepositWithdrawPaused('BTC')).toBe(true);
    expect(isDepositWithdrawPaused('POL')).toBe(false);
    expect(defaultDepositWithdrawCoin('BTC')).not.toBe('BTC');
    expect(isDepositWithdrawPaused(defaultDepositWithdrawCoin('BTC'))).toBe(false);
    expect(defaultDepositWithdrawCoin('POL')).toBe('POL');
  });
});

describe('safeBigInt', () => {
  it('handles nullish, number, string, scientific', () => {
    expect(safeBigInt(null)).toBe(0n);
    expect(safeBigInt(undefined)).toBe(0n);
    expect(safeBigInt('')).toBe(0n);
    expect(safeBigInt('0')).toBe(0n);
    expect(safeBigInt(12.9)).toBe(12n);
    expect(safeBigInt(NaN)).toBe(0n);
    expect(safeBigInt(Infinity)).toBe(0n);
    expect(safeBigInt('42')).toBe(42n);
    expect(safeBigInt('100.99')).toBe(100n);
    expect(safeBigInt(10n)).toBe(10n);
    expect(safeBigInt('1e+8')).toBe(100_000_000n);
    expect(safeBigInt('1.5e2')).toBe(150n);
    expect(safeBigInt('1.23e1')).toBe(12n); // exp < frac digits → Number trunc
    expect(safeBigInt('abce+2')).toBe(0n); // sci path throws → catch
    expect(safeBigInt('.')).toBe(0n);
    expect(safeBigInt('foo.bar')).toBe(0n); // decimal intPart BigInt throws
    expect(safeBigInt('not-a-number')).toBe(0n);
    expect(safeBigInt('1e-1')).toBe(0n); // negative exp, non-finite trunc path → fallthrough
  });
});

describe('computeSwap', () => {
  it('applies default fee bps', () => {
    const q = computeSwap('BTC', 'LTC', 100_000_000n, 100_000_000_000n, 1_000_000_000n, SWAP_DEFAULT_FEE_BPS);
    expect(q.feeBps).toBe(25);
    expect(q.toAmount).toBeGreaterThan(0n);
    expect(q.feeAmount).toBeGreaterThan(0n);
    expect(q.toAmount + q.feeAmount).toBeGreaterThan(q.toAmount);
  });

  it('rejects non-positive amount or price', () => {
    expect(() => computeSwap('BTC', 'LTC', 0n, 1n, 1n)).toThrow(/positive/i);
    expect(() => computeSwap('BTC', 'LTC', 1n, 0n, 1n)).toThrow(/Invalid price/i);
  });

  it('same price 1:1 roughly (ignoring fee)', () => {
    const q = computeSwap('USDT', 'USDC', 100_000_000n, 100_000_000n, 100_000_000n, 0);
    expect(q.toAmount).toBe(100_000_000n);
    expect(q.feeAmount).toBe(0n);
  });

  it('scales when toCoin has more decimals than fromCoin', () => {
    const usdt = COIN_CONFIG.USDT as { decimals: number };
    const prev = usdt.decimals;
    usdt.decimals = 6;
    try {
      const q = computeSwap('BTC', 'USDT', 100_000_000n, 80_000_0000_0000n, 1_0000_0000n, 0);
      expect(q.toAmount).toBeGreaterThan(0n);
    } finally {
      usdt.decimals = prev;
    }
  });

  it('scales when fromCoin has more decimals than toCoin', () => {
    const usdt = COIN_CONFIG.USDT as { decimals: number };
    const prev = usdt.decimals;
    usdt.decimals = 6;
    try {
      const q = computeSwap('USDT', 'BTC', 1_000_000n, 1_0000_0000n, 80_000_0000_0000n, 0);
      expect(q.toAmount).toBeGreaterThanOrEqual(0n);
    } finally {
      usdt.decimals = prev;
    }
  });
});

describe('formatAmount / parseAmount', () => {
  it('round-trips display strings', () => {
    expect(formatAmount(100_000_000n, 'BTC')).toBe('1');
    expect(formatAmount(1n, 'BTC')).toBe('0.00000001');
    expect(parseAmount('1.5', 'BTC')).toBe(150_000_000n);
    expect(parseAmount('1,5', 'BTC')).toBe(150_000_000n);
    expect(parseAmount('', 'BTC')).toBe(0n);
    expect(parseAmount('nope', 'BTC')).toBe(0n);
  });

  it('guards unknown coin casts', () => {
    const fake = 'ETH' as Coin;
    expect(formatAmount(1n, fake)).toBe('1');
    expect(parseAmount('1', fake)).toBe(0n);
    expect(getCoinUsdValue(1n, fake)).toBe(0);
  });
});

describe('getCoinUsdValue / formatUsdValue', () => {
  it('uses fallback prices', () => {
    const oneBtc = 100_000_000n;
    expect(getCoinUsdValue(oneBtc, 'BTC')).toBeCloseTo(FALLBACK_PRICES.BTC, 0);
    expect(getCoinUsdValue(0n, 'BTC')).toBe(0);
    expect(formatUsdValue(0n, 'BTC')).toBe('$0.00');
    expect(formatUsdValue(1n, 'BTC')).toMatch(/^\$|^< \$/);
  });

  it('accepts scaled price map', () => {
    const prices = { BTC: String(80_000 * 1e8) };
    const usd = getCoinUsdValue(100_000_000n, 'BTC', prices, 8);
    expect(usd).toBeCloseTo(80_000, 0);
    expect(formatUsdValue(100_000_000n, 'BTC', prices, 8)).toContain('80,000');
    // non-positive / NaN price entries fall back
    expect(getCoinUsdValue(100_000_000n, 'BTC', { BTC: '0' }, 8)).toBeCloseTo(FALLBACK_PRICES.BTC, 0);
    expect(getCoinUsdValue(100_000_000n, 'BTC', { BTC: 'nope' }, 8)).toBeCloseTo(FALLBACK_PRICES.BTC, 0);
  });

  it('formats dust tiers', () => {
    // tiny sat amount → very small USD with fallback BTC price
    expect(formatUsdValue(1n, 'DGB')).toMatch(/\$|</);
    expect(formatUsdValue(100n, 'DGB')).toMatch(/\$/);
  });
});

describe('formatLedgerAmount', () => {
  it('renders ledger units as a quantity of coins', () => {
    expect(formatLedgerAmount('2500000000', 'USDT')).toBe('25');
    expect(formatLedgerAmount('100000000', 'POL')).toBe('1');
    expect(formatLedgerAmount('1', 'BCH')).toBe('0.00000001');
    expect(formatLedgerAmount('5555555556', 'POL')).toBe('55.55555556');
  });

  it('handles the fractional rows written before the API rejected decimals', () => {
    // "7.2" here is 7.2 units of 1e-8 — the merchant dashboard used to print
    // it raw and claim the invoice charged 7.2 POL.
    expect(formatLedgerAmount('7.2000', 'POL')).toBe('0.000000072');
    expect(formatLedgerAmount('0.5000', 'USDC')).toBe('0.000000005');
  });

  it('never throws on junk', () => {
    expect(formatLedgerAmount('', 'BTC')).toBe('0');
    expect(formatLedgerAmount(null, 'BTC')).toBe('0');
    expect(formatLedgerAmount('abc', 'BTC')).toBe('0');
    expect(formatLedgerAmount('0', 'BTC')).toBe('0');
  });
});

import { describe, expect, it } from 'vitest';
import {
  coinsForMode,
  defaultPairForMode,
  DEX_SWAP_COINS,
  SWAP_COINS,
  asWalletBalances,
  computeSwap,
  formatAmount,
  formatAmountFixed,
  formatPortfolioUsd,
  formatUsdValue,
  getCoinUsdValue,
  isCoin,
  isDepositWithdrawPaused,
  isSwapL2Coin,
  isSwapL2Pair,
  isSwapCoin,
  isSwapPair,
  isSwapL1Coin,
  defaultDepositWithdrawCoin,
  parseAmount,
  safeBigInt,
  SWAP_DEFAULT_FEE_BPS,
  SWAP_L2_COINS,
  SWAP_L1_COINS,
  SWAP_COINS,
  DEX_SWAP_COINS,
  DEPOSIT_WITHDRAW_PAUSED_COINS,
  COINS,
  COIN_CONFIG,
  COIN_NETWORK,
  coinNetwork,
  isBridgePair,
  isDexSwapPair,
  isSameSwapNetwork,
  FALLBACK_PRICES,
  formatLedgerAmount,
  toLedgerUnits,
  type Coin,
} from '../../../src/shared/coins.js';

describe('isCoin / COINS', () => {
  it('accepts all configured symbols', () => {
    for (const c of COINS) expect(isCoin(c)).toBe(true);
    expect(isCoin('ETH')).toBe(false);
    expect(isCoin('')).toBe(false);
    expect(COINS).toHaveLength(11);
    expect(COINS).toContain('ZER');
    expect(COIN_CONFIG.ZER.name).toBe('Zero');
    expect(COIN_CONFIG.ZER.minConfirmations).toBe(10);
    expect(FALLBACK_PRICES.ZER).toBeGreaterThan(0);
    expect(COINS).toContain('PEPE');
    expect(COIN_CONFIG.PEPE.name).toBe('Pepe');
    expect(COIN_CONFIG.PEPE.minConfirmations).toBe(15);
    expect(COIN_CONFIG.PEPE.withdrawalFee).toBe(5_000_000_000_000n);
    expect(FALLBACK_PRICES.PEPE).toBeGreaterThan(0);
    expect(isDepositWithdrawPaused('PEPE')).toBe(false);
  });
});

describe('swap L2 allowlist', () => {
  it('allows only POL/USDT/USDC/SOL for L2', () => {
    expect(isSwapL2Coin('POL')).toBe(true);
    expect(isSwapL2Coin('BTC')).toBe(false);
    expect(isSwapL2Coin('PEPE')).toBe(false);
    expect(isSwapL2Pair('POL', 'USDT')).toBe(true);
    expect(isSwapL2Pair('POL', 'POL')).toBe(false);
    expect(isSwapL2Pair('BTC', 'LTC')).toBe(false);
    expect(SWAP_L2_COINS).toEqual(['POL', 'USDT', 'USDC', 'SOL']);
  });

  it('includes L1 coins in full swap allowlist', () => {
    expect(isSwapL1Coin('BTC')).toBe(true);
    expect(isSwapCoin('BTC')).toBe(true);
    expect(isSwapCoin('USDT')).toBe(true);
    expect(isSwapCoin('PEPE')).toBe(true);
    expect(isSwapPair('BTC', 'LTC')).toBe(true);
    expect(isSwapPair('BTC', 'USDT')).toBe(true);
    expect(isSwapPair('PEPE', 'USDT')).toBe(true);
    expect(isSwapPair('BTC', 'BTC')).toBe(false);
    expect(SWAP_L1_COINS).toEqual(['BTC', 'LTC', 'DOGE', 'BCH', 'DGB']);
    expect(SWAP_COINS).toContain('BTC');
    expect(SWAP_COINS).toContain('SOL');
    expect(SWAP_COINS).toContain('PEPE');
  });
});

describe('coin custodial networks', () => {
  it('marks USDT/USDC/POL as Polygon and PEPE as BSC', () => {
    expect(coinNetwork('USDT').id).toBe('polygon');
    expect(coinNetwork('USDC').short).toBe('Polygon');
    expect(coinNetwork('POL').id).toBe('polygon');
    expect(coinNetwork('SOL').id).toBe('solana');
    expect(coinNetwork('PEPE').id).toBe('bsc');
    expect(COIN_NETWORK.BTC.id).toBe('bitcoin');
  });

  it('separates same-network DEX swap from cross-network bridge', () => {
    expect(DEX_SWAP_COINS).toEqual(['POL', 'USDT', 'USDC']);
    expect(isDexSwapPair('POL', 'USDT')).toBe(true);
    expect(isSameSwapNetwork('USDT', 'USDC')).toBe(true);
    expect(isBridgePair('POL', 'USDT')).toBe(false);
    expect(isBridgePair('SOL', 'USDT')).toBe(true);
    expect(isBridgePair('BTC', 'LTC')).toBe(true);
    expect(isBridgePair('PEPE', 'USDT')).toBe(true);
    expect(isDexSwapPair('SOL', 'USDT')).toBe(false);
    expect(isDexSwapPair('PEPE', 'USDT')).toBe(false);
  });
});

describe('deposit/withdraw pause list', () => {
  it('pauses BTC LTC DOGE BCH DGB but keeps them as coins', () => {
    expect(DEPOSIT_WITHDRAW_PAUSED_COINS).toEqual(['BTC', 'LTC', 'DOGE', 'BCH', 'DGB']);
    expect(isDepositWithdrawPaused('BTC')).toBe(true);
    expect(isDepositWithdrawPaused('DGB')).toBe(true);
    expect(isDepositWithdrawPaused('ZER')).toBe(false);
    expect(isDepositWithdrawPaused('BCH')).toBe(true);
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
    expect(q.feeBps).toBe(SWAP_DEFAULT_FEE_BPS);
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
    expect(formatAmountFixed(1n, fake)).toBe('1');
    expect(parseAmount('1', fake)).toBe(0n);
    expect(getCoinUsdValue(1n, fake)).toBe(0);
  });

  it('formatAmountFixed always keeps 8 fraction digits', () => {
    expect(formatAmountFixed(100_000_000n, 'BTC')).toBe('1.00000000');
    expect(formatAmountFixed(1n, 'BTC')).toBe('0.00000001');
    expect(formatAmountFixed(0n, 'ZER')).toBe('0.00000000');
    expect(formatAmountFixed(800_000_000n, 'ZER')).toBe('8.00000000');
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
    // Never show bare $0.00 for a non-zero ledger credit
    expect(formatUsdValue(1n, 'BTC')).not.toBe('$0.00');
    expect(formatPortfolioUsd(0, true)).toBe('< $0.01');
    expect(formatPortfolioUsd(0.004, true)).toBe('< $0.01');
    expect(formatPortfolioUsd(0.004, false)).toBe('< $0.01');
    expect(formatPortfolioUsd(0, false)).toBe('$0.00');
    expect(formatPortfolioUsd(12.5, false)).toContain('12.50');
  });
});

describe('asWalletBalances', () => {
  it('accepts bare array or { wallets } wrapper', () => {
    const row = { coin: 'BTC' as const, balance: '1', kind: 'PERSONAL' };
    expect(asWalletBalances([row])).toEqual([row]);
    expect(asWalletBalances({ wallets: [row] })).toEqual([row]);
    expect(asWalletBalances(null)).toEqual([]);
    expect(asWalletBalances({ wallets: undefined })).toEqual([]);
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

describe('toLedgerUnits', () => {
  it('converts a coin quantity into the integer the API takes', () => {
    // The spelling a merchant reaches for ("25.00") is the one that broke a
    // real integration; this is the conversion that makes it safe.
    expect(toLedgerUnits('25')).toBe('2500000000');
    expect(toLedgerUnits('25.00')).toBe('2500000000');
    expect(toLedgerUnits('0.005')).toBe('500000');
    expect(toLedgerUnits('0.00000001')).toBe('1');
    expect(toLedgerUnits('25,5')).toBe('2550000000');
  });

  it('refuses what the ledger cannot hold', () => {
    expect(toLedgerUnits('0.000000001')).toBeNull(); // 9 decimals
    expect(toLedgerUnits('abc')).toBeNull();
    expect(toLedgerUnits('')).toBeNull();
    expect(toLedgerUnits('-1')).toBeNull();
    expect(toLedgerUnits('0')).toBeNull();
  });

  it('round-trips with formatLedgerAmount', () => {
    for (const coins of ['25', '0.005', '1', '0.00000001']) {
      expect(formatLedgerAmount(toLedgerUnits(coins)!, 'USDT')).toBe(coins.replace(/^(\d+)\.?0*$/, '$1'));
    }
  });
});

describe('swap mode helpers', () => {
  it('swap tab lists DEX coins, bridge tab lists every swap coin', () => {
    expect(coinsForMode('swap')).toBe(DEX_SWAP_COINS);
    expect(coinsForMode('bridge')).toBe(SWAP_COINS);
  });

  it('default pair is a valid pair for its own mode', () => {
    expect(defaultPairForMode('swap')).toEqual({ from: 'POL', to: 'USDT' });
    expect(defaultPairForMode('bridge')).toEqual({ from: 'SOL', to: 'USDT' });
  });
});


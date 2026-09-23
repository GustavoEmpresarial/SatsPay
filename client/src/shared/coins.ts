export const COINS = ['BTC', 'LTC', 'DOGE', 'BCH', 'POL', 'DGB', 'SOL', 'USDT', 'USDC', 'ZER', 'PEPE'] as const;
export type Coin = (typeof COINS)[number];

/** Polygon L2 + SOL (Relay bridge) allowed for custodial DEX swap. */
export const SWAP_L2_COINS = ['POL', 'USDT', 'USDC', 'SOL'] as const;
export type SwapL2Coin = (typeof SWAP_L2_COINS)[number];

/** L1 UTXO coins swapped via ChangeNOW. */
export const SWAP_L1_COINS = ['BTC', 'LTC', 'DOGE', 'BCH', 'DGB'] as const;
export type SwapL1Coin = (typeof SWAP_L1_COINS)[number];

/** Full swap picker: L1 + L2/SOL (SOL perto do topo — fácil achar). */
export const SWAP_COINS = [
  'BTC',
  'SOL',
  'LTC',
  'DOGE',
  'BCH',
  'DGB',
  'POL',
  'USDT',
  'USDC',
  'PEPE',
] as const;
export type SwapCoin = (typeof SWAP_COINS)[number];

/**
 * Same-network DEX swap (aba Swap): só Polygon — POL ↔ USDT ↔ USDC.
 * Bridge (cross-rede) usa o allowlist completo `SWAP_COINS` (inclui PEPE na BSC via Relay).
 */
export const DEX_SWAP_COINS = ['POL', 'USDT', 'USDC'] as const;
export type DexSwapCoin = (typeof DEX_SWAP_COINS)[number];

export type SwapMode = 'swap' | 'bridge';

/**
 * Custodial network for each coin on SatsPay (deposit / withdraw / swap legs).
 * USDT/USDC/POL = Polygon PoS — not Ethereum, not BSC.
 * PEPE = BNB Smart Chain BEP-20 — not Ethereum PEPE.
 */
export type CoinNetworkId =
  | 'bitcoin'
  | 'litecoin'
  | 'dogecoin'
  | 'bitcoincash'
  | 'digibyte'
  | 'polygon'
  | 'solana'
  | 'zero'
  | 'bsc';

export interface CoinNetwork {
  id: CoinNetworkId;
  /** Short badge in pickers (e.g. "Polygon", "BSC"). */
  short: string;
  /** Full label for tooltips / copy. */
  label: string;
}

export const COIN_NETWORK: Record<Coin, CoinNetwork> = {
  BTC: { id: 'bitcoin', short: 'Bitcoin', label: 'Bitcoin' },
  LTC: { id: 'litecoin', short: 'Litecoin', label: 'Litecoin' },
  DOGE: { id: 'dogecoin', short: 'Dogecoin', label: 'Dogecoin' },
  BCH: { id: 'bitcoincash', short: 'Bitcoin Cash', label: 'Bitcoin Cash' },
  DGB: { id: 'digibyte', short: 'DigiByte', label: 'DigiByte' },
  POL: { id: 'polygon', short: 'Polygon', label: 'Polygon PoS' },
  USDT: { id: 'polygon', short: 'Polygon', label: 'Polygon PoS (USDT bridged)' },
  USDC: { id: 'polygon', short: 'Polygon', label: 'Polygon PoS (USDC native)' },
  SOL: { id: 'solana', short: 'Solana', label: 'Solana' },
  ZER: { id: 'zero', short: 'Zero', label: 'Zero (transparent t1)' },
  PEPE: { id: 'bsc', short: 'BSC', label: 'BNB Smart Chain (BEP-20)' },
};

export function coinNetwork(coin: Coin): CoinNetwork {
  return COIN_NETWORK[coin];
}

export function isDexSwapCoin(value: string): value is DexSwapCoin {
  return (DEX_SWAP_COINS as readonly string[]).includes(value);
}

/** True when both coins share the same custodial network (e.g. POL+USDT). */
export function isSameSwapNetwork(from: string, to: string): boolean {
  if (!isCoin(from) || !isCoin(to)) return false;
  return coinNetwork(from).id === coinNetwork(to).id;
}

/** Aba Swap: par DEX na mesma rede (hoje só Polygon). */
export function isDexSwapPair(from: string, to: string): boolean {
  return from !== to && isDexSwapCoin(from) && isDexSwapCoin(to);
}

/** Aba Bridge: par allowlisted em redes distintas. */
export function isBridgePair(from: string, to: string): boolean {
  return from !== to && isSwapCoin(from) && isSwapCoin(to) && !isSameSwapNetwork(from, to);
}

export function coinsForMode(mode: SwapMode): readonly Coin[] {
  return mode === 'swap' ? DEX_SWAP_COINS : SWAP_COINS;
}

export function defaultPairForMode(mode: SwapMode): { from: Coin; to: Coin } {
  return mode === 'swap' ? { from: 'POL', to: 'USDT' } : { from: 'SOL', to: 'USDT' };
}

/**
 * Temporary pause: personal deposit/withdraw pickers still list paused networks,
 * but those rows are disabled (not selectable). Address generation, withdrawals,
 * and merchant deposit gateway invoices are blocked by the API.
 * `/v1/public/send` is NOT paused. Custodial ChangeNOW swaps from balance ARE allowed.
 */
export const DEPOSIT_WITHDRAW_PAUSED_COINS = ['BTC', 'LTC', 'DOGE', 'BCH', 'DGB'] as const;
export type DepositWithdrawPausedCoin = (typeof DEPOSIT_WITHDRAW_PAUSED_COINS)[number];

export function isSwapL2Coin(value: string): value is SwapL2Coin {
  return (SWAP_L2_COINS as readonly string[]).includes(value);
}

export function isSwapL1Coin(value: string): value is SwapL1Coin {
  return (SWAP_L1_COINS as readonly string[]).includes(value);
}

export function isSwapCoin(value: string): value is SwapCoin {
  return (SWAP_COINS as readonly string[]).includes(value);
}

export function isSwapL2Pair(from: string, to: string): boolean {
  return from !== to && isSwapL2Coin(from) && isSwapL2Coin(to);
}

export function isSwapPair(from: string, to: string): boolean {
  return from !== to && isSwapCoin(from) && isSwapCoin(to);
}

export function isDepositWithdrawPaused(value: string): boolean {
  return (DEPOSIT_WITHDRAW_PAUSED_COINS as readonly string[]).includes(value.toUpperCase());
}

/** Coins the user can pick for personal deposit / withdraw right now. */
export function depositWithdrawActiveCoins(): Coin[] {
  return COINS.filter((c) => !isDepositWithdrawPaused(c));
}

/** Paused networks — picker rows, disabled. */
export function depositWithdrawPausedCoinList(): Coin[] {
  return COINS.filter((c) => isDepositWithdrawPaused(c));
}

/** Prefer URL coin when active; otherwise first non-paused coin. */
export function defaultDepositWithdrawCoin(preferred?: string | null): Coin {
  const raw = preferred?.toUpperCase();
  if (raw && isCoin(raw) && !isDepositWithdrawPaused(raw)) return raw;
  return COINS.find((c) => !isDepositWithdrawPaused(c)) ?? 'POL';
}

/** Internal ledger scale — every coin is stored with 8 decimal places. */
export const INTERNAL_AMOUNT_DECIMALS = 8;
/** Default swap fee in basis points — SatsPay diferencial (0.25%). */
export const SWAP_DEFAULT_FEE_BPS = 25;

export function isCoin(value: string): value is Coin {
  return COINS.some((c) => c === value);
}

export interface CoinConfig {
  symbol: Coin;
  name: string;
  decimals: number;
  minWithdrawal: bigint;
  withdrawalFee: bigint;
  faucetPayFee: bigint;
  minConfirmations: number;
  faucetReward: bigint;
  displayColor: string;
  /**
   * Amounts >= this (in the coin's smallest unit) require manual admin approval.
   */
  approvalThreshold: bigint;
}

export const COIN_CONFIG: Record<Coin, CoinConfig> = {
  BTC: {
    symbol: 'BTC',
    name: 'Bitcoin',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 1_000n,   // 0.00001000 BTC — FaucetPay saque normal
    faucetPayFee: 1_000n,
    minConfirmations: 2,
    faucetReward: 1n,        // 0.00000001 BTC — near-zero
    displayColor: '#F7931A',
    approvalThreshold: 1_500_000n, // 0.015 BTC (~$1,000 USD)
  },
  LTC: {
    symbol: 'LTC',
    name: 'Litecoin',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 2_000n,   // 0.00002000 LTC — FaucetPay saque normal
    faucetPayFee: 2_000n,
    minConfirmations: 6,
    faucetReward: 1n,        // 0.00000001 LTC — near-zero
    displayColor: '#345D9D',
    approvalThreshold: 1_000_000_000n, // 10 LTC (~$1,000 USD)
  },
  DOGE: {
    symbol: 'DOGE',
    name: 'Dogecoin',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 100_000_000n,   // 1 DOGE — FaucetPay saque normal
    faucetPayFee: 100_000_000n,
    minConfirmations: 20,
    faucetReward: 1n,       // 0.00000001 DOGE — near-zero
    displayColor: '#C2A633',
    approvalThreshold: 600_000_000_000n, // 6,000 DOGE (~$1,000 USD)
  },
  BCH: {
    symbol: 'BCH',
    name: 'Bitcoin Cash',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 5_000n,  // 0.00005000 BCH — FaucetPay saque normal
    faucetPayFee: 5_000n,
    minConfirmations: 2,
    faucetReward: 1n,        // 0.00000001 BCH — near-zero
    displayColor: '#0AC18E',
    approvalThreshold: 250_000_000n, // 2.5 BCH (~$1,000 USD)
  },
  POL: {
    symbol: 'POL',
    name: 'Polygon',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 3_000_000n, // 0.03000000 POL — FaucetPay saque normal
    faucetPayFee: 3_000_000n,
    minConfirmations: 30,
    faucetReward: 1n,          // 0.00000001 POL — near-zero
    displayColor: '#8247E5',
    approvalThreshold: 250_000_000_000n, // 2,500 POL (~$1,000 USD)
  },
  DGB: {
    symbol: 'DGB',
    name: 'DigiByte',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 25_000_000n,    // 0.25000000 DGB — FaucetPay saque normal
    faucetPayFee: 25_000_000n,
    minConfirmations: 40,
    faucetReward: 1n,            // 0.00000001 DGB — near-zero
    displayColor: '#0066CC',
    approvalThreshold: 12_000_000_000_000n, // 120,000 DGB (~$1,000 USD)
  },
  SOL: {
    symbol: 'SOL',
    name: 'Solana',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 10_000n, // 0.00010000 SOL — FaucetPay saque normal
    faucetPayFee: 10_000n,
    minConfirmations: 32,
    faucetReward: 1n,              // 0.00000001 SOL — near-zero
    displayColor: '#14F195',
    approvalThreshold: 600_000_000n, // 6 SOL (~$1,000 USD)
  },
  USDT: {
    symbol: 'USDT',
    name: 'Tether USD',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 1_000_000n, // 0.01000000 USDT
    faucetPayFee: 1_000_000n,
    minConfirmations: 30,
    faucetReward: 1n,            // 0.00000001 USDT — near-zero
    displayColor: '#26A17B',
    approvalThreshold: 100_000_000_000n, // 1,000 USDT ($1,000 USD)
  },
  USDC: {
    symbol: 'USDC',
    name: 'USD Coin',
    decimals: 8,
    minWithdrawal: 1n, // no minimum — 1 atomic unit
    withdrawalFee: 1_000_000n, // 0.01000000 USDC
    faucetPayFee: 1_000_000n,
    minConfirmations: 30,
    faucetReward: 1n,            // 0.00000001 USDC — near-zero
    displayColor: '#2775CA',
    approvalThreshold: 100_000_000_000n, // 1,000 USDC ($1,000 USD)
  },
  ZER: {
    symbol: 'ZER',
    name: 'Zero',
    decimals: 8,
    minWithdrawal: 1n,
    withdrawalFee: 100_000n, // 0.001 ZER
    faucetPayFee: 100_000n,
    minConfirmations: 10,
    faucetReward: 1n,
    displayColor: '#1a1a1a',
    approvalThreshold: 10_000_000_000_000n, // 100,000 ZER ≈ $1,000
  },
  PEPE: {
    symbol: 'PEPE',
    name: 'Pepe',
    decimals: 8,
    minWithdrawal: 1n,
    withdrawalFee: 5_000_000_000_000n, // 50,000 PEPE — gas on-chain is BNB
    faucetPayFee: 5_000_000_000_000n,
    minConfirmations: 15,
    faucetReward: 1n,
    displayColor: '#3CB43C',
    approvalThreshold: 25_000_000_000_000_000n, // 250,000,000 PEPE ≈ $1,000
  },
};

export interface SwapQuote {
  fromCoin: Coin;
  toCoin: Coin;
  fromAmount: bigint;
  toAmount: bigint;
  feeAmount: bigint;
  feeBps: number;
  priceFrom: bigint;
  priceTo: bigint;
  priceDecimals: number;
}

export function computeSwap(
  fromCoin: Coin,
  toCoin: Coin,
  fromAmount: bigint,
  priceFrom: bigint,
  priceTo: bigint,
  feeBps: number = SWAP_DEFAULT_FEE_BPS,
  _priceDecimals: number = INTERNAL_AMOUNT_DECIMALS,
): SwapQuote {
  if (fromAmount <= 0n) throw new Error('Amount must be positive');
  if (priceFrom <= 0n || priceTo <= 0n) throw new Error('Invalid price');

  const fromDec = BigInt(COIN_CONFIG[fromCoin].decimals);
  const toDec = BigInt(COIN_CONFIG[toCoin].decimals);

  const fromValue = fromAmount * priceFrom;
  let rawToAmount: bigint;
  if (fromDec >= toDec) {
    rawToAmount = fromValue / (priceTo * 10n ** (fromDec - toDec));
  } else {
    rawToAmount = (fromValue * 10n ** (toDec - fromDec)) / priceTo;
  }

  const BPS_DENOMINATOR = 10_000n;
  const feeAmount = (rawToAmount * BigInt(feeBps)) / BPS_DENOMINATOR;
  const toAmount = rawToAmount - feeAmount;

  return {
    fromCoin,
    toCoin,
    fromAmount,
    toAmount,
    feeAmount,
    feeBps,
    priceFrom,
    priceTo,
    priceDecimals: INTERNAL_AMOUNT_DECIMALS,
  };
}

export function safeBigInt(val: bigint | number | string | null | undefined): bigint {
  if (val === null || val === undefined || val === '') return 0n;
  if (typeof val === 'bigint') return val;
  if (typeof val === 'number') {
    if (!Number.isFinite(val) || isNaN(val)) return 0n;
    return BigInt(Math.trunc(val));
  }
  const s = String(val).trim();
  if (!s || s === '0') return 0n;

  // Handle scientific notation e.g. "100e+16", "1e+8", "1.5e18"
  if (/[eE]/.test(s)) {
    try {
      const parts = s.split(/[eE]/);
      const mantissa = parts[0] || '0';
      const expStr = parts[1] || '0';
      const exp = parseInt(expStr, 10);
      if (exp >= 0) {
        const [intPart = '0', fracPart = ''] = mantissa.split('.');
        if (exp >= fracPart.length) {
          const combined = (intPart + fracPart + '0'.repeat(exp - fracPart.length)).replace(/^0+/, '');
          return BigInt(combined || '0');
        }
      }
      const num = Number(s);
      if (Number.isFinite(num)) {
        return BigInt(Math.trunc(num));
      }
    } catch {
      // fallback to 0n
    }
  }

  // Handle decimal dot strings e.g. "100.00" -> take integer part
  if (s.includes('.')) {
    const intPart = s.split('.')[0] || '0';
    try {
      return BigInt(intPart);
    } catch {
      return 0n;
    }
  }

  try {
    return BigInt(s);
  } catch {
    return 0n;
  }
}

export function formatAmount(amount: bigint | number | string | null | undefined, coin: Coin): string {
  const cfg = COIN_CONFIG[coin];
  if (!cfg) return String(amount ?? 0);

  const raw = safeBigInt(amount);
  const factor = 10n ** BigInt(cfg.decimals);
  const whole = raw / factor;
  const frac = raw % factor;

  if (frac === 0n) {
    return whole.toString();
  }

  const fracStr = frac.toString().padStart(cfg.decimals, '0').replace(/0+$/, '');
  return `${whole}.${fracStr}`;
}

/** Wallet display: always `decimals` fraction digits (ledger scale is 8). */
export function formatAmountFixed(amount: bigint | number | string | null | undefined, coin: Coin): string {
  const cfg = COIN_CONFIG[coin];
  if (!cfg) return String(amount ?? 0);

  const raw = safeBigInt(amount);
  const places = cfg.decimals;
  const factor = 10n ** BigInt(places);
  const whole = raw / factor;
  const frac = raw % factor;
  return `${whole}.${frac.toString().padStart(places, '0')}`;
}

export function parseAmount(display: string, coin: Coin): bigint {
  const cfg = COIN_CONFIG[coin];
  if (!cfg) return 0n;

  const clean = display.trim().replace(',', '.');
  if (!clean || isNaN(Number(clean))) return 0n;

  const [wholeStr, fracStr = ''] = clean.split('.');
  const whole = safeBigInt(wholeStr || '0');
  const paddedFrac = fracStr.padEnd(cfg.decimals, '0').slice(0, cfg.decimals);
  const frac = safeBigInt(paddedFrac);

  return whole * 10n ** BigInt(cfg.decimals) + frac;
}

export const FALLBACK_PRICES: Record<Coin, number> = {
  BTC: 79250.0,
  LTC: 92.5,
  DOGE: 0.12,
  BCH: 385.0,
  POL: 0.42,
  DGB: 0.0095,
  SOL: 148.0,
  USDT: 1.0,
  USDC: 1.0,
  ZER: 0.01,
  PEPE: 0.000004,
};

export function getCoinUsdValue(
  amount: bigint | number | string | null | undefined,
  coin: Coin,
  prices?: Record<string, string | number>,
  priceDecimals = 8,
): number {
  const raw = safeBigInt(amount);
  if (raw === 0n) return 0;
  const cfg = COIN_CONFIG[coin];
  if (!cfg) return 0;

  const coinAmount = Number(raw) / 10 ** cfg.decimals;
  let price = FALLBACK_PRICES[coin] ?? 1.0;
  if (prices && prices[coin] !== undefined) {
    const rawP = Number(prices[coin]);
    if (Number.isFinite(rawP) && rawP > 0) {
      price = rawP / 10 ** priceDecimals;
    }
  }
  return coinAmount * price;
}

export function formatUsdValue(
  amount: bigint | number | string | null | undefined,
  coin: Coin,
  prices?: Record<string, string | number>,
  priceDecimals = 8,
): string {
  const usd = getCoinUsdValue(amount, coin, prices, priceDecimals);
  const raw = safeBigInt(amount);
  // Non-zero ledger dust must never look like a false $0.00 credit.
  if (raw !== 0n && usd === 0) return '< $0.0001';
  if (usd === 0) return '$0.00';
  if (usd > 0 && usd < 0.0001) {
    return '< $0.0001';
  }
  if (usd > 0 && usd < 0.01) {
    return `$${usd.toFixed(4)}`;
  }
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(usd);
}

export interface WalletBalance {
  id?: string;
  coin: Coin;
  balance: string;
  lockedBalance?: string;
  kind?: string;
  address?: string | null;
}

/** Portfolio / KPI hero: never show bare `$0.00` when there is a non-zero balance. */
export function formatPortfolioUsd(totalUsd: number, hasNonZeroBalance: boolean): string {
  if (hasNonZeroBalance && (totalUsd <= 0 || totalUsd < 0.01)) {
    return '< $0.01';
  }
  if (totalUsd > 0 && totalUsd < 0.01) {
    return '< $0.01';
  }
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(totalUsd);
}

/** API may return a bare array or `{ wallets }` — normalize like Dashboard/Checkout. */
export function asWalletBalances(
  data: WalletBalance[] | { wallets?: WalletBalance[] } | null | undefined,
): WalletBalance[] {
  if (!data) return [];
  if (Array.isArray(data)) return data;
  return Array.isArray(data.wallets) ? data.wallets : [];
}

/**
 * Renders a ledger amount as a quantity of coins.
 *
 * Unlike `formatAmount`, this tolerates a fractional ledger value: rows
 * written before the API rejected decimal `amount` hold things like "7.2"
 * (7.2 units of 1e-8, not 7.2 coins). Showing the raw value told the merchant
 * they had charged 7.2 POL when the invoice actually asked for 0.000000072.
 */
export function formatLedgerAmount(raw: string | number | null | undefined, _coin: Coin): string {
  const text = String(raw ?? '').trim();
  if (!text || !/^\d*\.?\d*$/.test(text)) return '0';

  const [whole = '0', frac = ''] = text.split('.');
  const scale = INTERNAL_AMOUNT_DECIMALS;
  // Shift the decimal point left by `scale` places, with string math so no
  // precision is lost on large amounts.
  const digits = `${whole}${frac}`.replace(/^0+(?=\d)/, '') || '0';
  const pointFromRight = frac.length + scale;
  const padded = digits.padStart(pointFromRight + 1, '0');
  const head = padded.slice(0, padded.length - pointFromRight) || '0';
  const tail = padded.slice(padded.length - pointFromRight).replace(/0+$/, '');
  return tail ? `${head}.${tail}` : head;
}

/**
 * "25" (coins) → "2500000000" (ledger units of 1e-8), or null when the input
 * is not a quantity the ledger can hold. The inverse of
 * `formatLedgerAmount`. String math on purpose: floats lose the last satoshi.
 */
export function toLedgerUnits(input: string): string | null {
  const t = input.trim().replace(',', '.');
  if (t === '' || t === '.' || !/^\d*\.?\d*$/.test(t)) return null;
  const [whole = '0', frac = ''] = t.split('.');
  if (frac.length > INTERNAL_AMOUNT_DECIMALS) return null;
  const units = `${whole}${frac.padEnd(INTERNAL_AMOUNT_DECIMALS, '0')}`.replace(/^0+(?=\d)/, '');
  return units === '' || /^0+$/.test(units) ? null : units;
}

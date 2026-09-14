import { COIN_CONFIG, safeBigInt, type Coin } from '@/shared';

/**
 * Parse a human decimal string ("0.001") into ledger smallest units.
 * Returns 0n for empty/invalid input.
 */
export function parseHumanAmount(input: string, coin: Coin): bigint {
  const trimmed = input.trim();
  if (!trimmed || !/^\d+(\.\d+)?$/.test(trimmed)) return 0n;
  const n = Number(trimmed);
  if (!Number.isFinite(n) || n <= 0) return 0n;

  const decimals = COIN_CONFIG[coin]?.decimals ?? 8;
  const parts = trimmed.split('.');
  const whole = safeBigInt(parts[0] || '0') * 10n ** BigInt(decimals);
  if (parts.length > 1) {
    const fracStr = (parts[1] ?? '').slice(0, decimals).padEnd(decimals, '0');
    return whole + safeBigInt(fracStr);
  }
  return whole;
}

/** Net amount after subtracting withdrawal fee (never negative). */
export function netAfterWithdrawalFee(amount: bigint, fee: bigint): bigint {
  if (amount <= fee) return 0n;
  return amount - fee;
}

/** Whether amount meets coin min withdrawal and has enough balance including fee. */
export function canWithdraw(opts: {
  amount: bigint;
  balance: bigint;
  fee: bigint;
  minWithdrawal: bigint;
}): { ok: boolean; reason?: 'zero' | 'belowMin' | 'insufficient' } {
  const { amount, balance, fee, minWithdrawal } = opts;
  if (amount <= 0n) return { ok: false, reason: 'zero' };
  if (amount < minWithdrawal) return { ok: false, reason: 'belowMin' };
  if (amount + fee > balance) return { ok: false, reason: 'insufficient' };
  return { ok: true };
}

/** Basic non-empty address check (chain-specific validation is server-side). */
export function isPlausibleAddress(address: string): boolean {
  const a = address.trim();
  if (a.length < 10 || a.length > 128) return false;
  if (/\s/.test(a)) return false;
  return true;
}

/**
 * User-facing analytics buckets for ledger rows.
 * Admin seeds (ADJUSTMENT) and pure bookkeeping noise stay out of the chart.
 */

const ANALYTICS_INFLOW = new Set([
  'DEPOSIT',
  'FAUCET',
  'TRANSFER_IN',
  'MERCHANT_DEPOSIT',
  'WITHDRAWAL_REVERSAL',
  'DEX_SWAP_REFUND',
  'STAKE_UNLOCK',
  'STAKE_REWARD',
  'SWAP_IN',
]);

const ANALYTICS_OUTFLOW = new Set([
  'WITHDRAWAL',
  'WITHDRAWAL_FEE',
  'TRANSFER_OUT',
  'MERCHANT_CHECKOUT',
  'DEX_SWAP_OUT',
  'SWAP_OUT',
  'STAKE_LOCK',
]);

/** Skip entirely (admin seed / internal noise). */
const ANALYTICS_SKIP = new Set(['ADJUSTMENT']);

/**
 * Map raw ledger type → display bucket.
 * Returns null when the row must not appear in “Por operação”.
 * Sign: +1 adds volume to the bucket, -1 nets against it (reversal/refund).
 */
export function analyticsBucket(type: string): { bucket: string; sign: 1 | -1 } | null {
  if (ANALYTICS_SKIP.has(type)) return null;
  switch (type) {
    case 'MERCHANT_DEPOSIT':
      return { bucket: 'DEPOSIT', sign: 1 };
    case 'WITHDRAWAL_REVERSAL':
      return { bucket: 'WITHDRAWAL', sign: -1 };
    case 'DEX_SWAP_OUT':
    case 'SWAP_OUT':
      return { bucket: 'SWAP', sign: 1 };
    case 'DEX_SWAP_REFUND':
    case 'SWAP_IN':
      return { bucket: 'SWAP', sign: -1 };
    default:
      return { bucket: type, sign: 1 };
  }
}

export type AnalyticsLedgerRow = {
  type: string;
  amount: string | number | bigint;
  /** Precomputed absolute USD for |amount|. */
  absUsd: number;
};

export type AnalyticsByTypeRow = {
  type: string;
  count: number;
  usd: number;
  inflow: boolean;
};

/** Aggregate absolute USD by user-facing bucket; reversals net against the parent. */
export function aggregateByType(rows: AnalyticsLedgerRow[]): AnalyticsByTypeRow[] {
  const m = new Map<string, { count: number; usd: number }>();
  for (const e of rows) {
    const mapped = analyticsBucket(e.type);
    if (!mapped) continue;
    const cur = m.get(mapped.bucket) ?? { count: 0, usd: 0 };
    cur.count += 1;
    cur.usd += mapped.sign * e.absUsd;
    m.set(mapped.bucket, cur);
  }
  return Array.from(m.entries())
    .map(([type, v]) => ({
      type,
      count: v.count,
      usd: Math.max(0, v.usd),
      inflow: ANALYTICS_INFLOW.has(type) || type === 'DEPOSIT' || type === 'FAUCET' || type === 'TRANSFER_IN',
    }))
    .filter((r) => r.count > 0 && r.usd >= 0)
    .sort((a, b) => b.usd - a.usd);
}

export function isAnalyticsInflow(type: string, amountPositive: boolean): boolean {
  if (ANALYTICS_SKIP.has(type)) return false;
  if (ANALYTICS_INFLOW.has(type)) return true;
  if (ANALYTICS_OUTFLOW.has(type)) return false;
  return amountPositive;
}

export function isAnalyticsOutflow(type: string, amountPositive: boolean): boolean {
  if (ANALYTICS_SKIP.has(type)) return false;
  if (ANALYTICS_OUTFLOW.has(type)) return true;
  if (ANALYTICS_INFLOW.has(type)) return false;
  return !amountPositive;
}

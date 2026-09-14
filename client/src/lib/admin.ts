import { COINS, formatAmount, isCoin, safeBigInt, type Coin } from '@/shared';

export interface TreasuryBalanceInput {
  coin: string;
  onchain: string;
  ledger: string;
  error: string | null;
}

export interface AggregatedCoinBalance {
  coin: string;
  onchain: string;
  ledger: string;
  addressCount: number;
  error: string | null;
}

/** Sum on-chain and ledger per coin. Unknown tickers follow the live coin order, then extras. */
export function aggregateTreasuryByCoin(rows: TreasuryBalanceInput[]): AggregatedCoinBalance[] {
  const totals = new Map<string, { onchain: bigint; ledger: bigint; addressCount: number; error: string | null }>();
  for (const row of rows) {
    const cur = totals.get(row.coin) ?? { onchain: 0n, ledger: 0n, addressCount: 0, error: null };
    cur.onchain += safeBigInt(row.onchain);
    cur.ledger += safeBigInt(row.ledger);
    cur.addressCount += 1;
    if (!cur.error && row.error) cur.error = row.error;
    totals.set(row.coin, cur);
  }

  const extras = [...totals.keys()].filter((coin) => !isCoin(coin));
  const order = [...COINS.filter((coin) => totals.has(coin)), ...extras];
  return order.map((coin) => {
    const t = totals.get(coin)!;
    return {
      coin,
      onchain: t.onchain.toString(),
      ledger: t.ledger.toString(),
      addressCount: t.addressCount,
      error: t.error,
    };
  });
}

/** Hide zero rows — admin only needs coins that actually hold value. */
export function treasuryRowsWithBalance(rows: AggregatedCoinBalance[]): AggregatedCoinBalance[] {
  return rows.filter((row) => safeBigInt(row.onchain) > 0n || safeBigInt(row.ledger) > 0n);
}

/** UI truncation for hashes/addresses — prefix/suffix character counts. */
export const HASH_PREFIX_CHARS = 8;
export const HASH_SUFFIX_CHARS = 6;
/** Max RPC error characters shown before ellipsis (3-char ellipsis). */
export const RPC_ERROR_PREVIEW_MAX_CHARS = 80;
const RPC_ERROR_ELLIPSIS_CHARS = 1;

const WITHDRAWAL_STATUS_LABELS: Record<string, string> = {
  PENDING: 'Aguardando aprovação',
  APPROVED: 'Aprovado',
  QUEUED: 'Na fila',
  BROADCASTING: 'Transmitindo',
  BROADCASTED: 'Transmitido',
  CONFIRMED: 'Confirmado',
  FAILED: 'Falhou (estornado)',
  CANCELED: 'Cancelado',
  CANCELLED: 'Cancelado',
  REJECTED: 'Cancelado',
};

export function parseCoin(coin: string): Coin | undefined {
  return isCoin(coin) ? coin : undefined;
}

export function formatAdminAmount(amount: string | number | null | undefined, coin: string): string {
  const parsed = parseCoin(coin);
  if (!parsed) return `${amount ?? 0} ${coin}`;
  return `${formatAmount(amount, parsed)} ${coin}`;
}

export function formatAdminDate(value: string | number | Date | null | undefined): string {
  if (value == null || value === '') return '—';
  const d = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(d.getTime())) return '—';
  return d.toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'short' });
}

export function formatAdminTime(value: string | number | Date | null | undefined): string {
  if (value == null || value === '') return '—';
  const d = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(d.getTime())) return '—';
  return d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export function pickDate(...values: Array<string | number | Date | null | undefined>): string | undefined {
  for (const v of values) {
    if (v == null || v === '') continue;
    const d = v instanceof Date ? v : new Date(v);
    if (!Number.isNaN(d.getTime())) return v instanceof Date ? v.toISOString() : String(v);
  }
  return undefined;
}

export function explorerAddressUrl(coin: string, address: string): string {
  switch (coin) {
    case 'BTC':
      return `https://mempool.space/address/${address}`;
    case 'LTC':
      return `https://litecoinspace.org/address/${address}`;
    case 'DOGE':
      return `https://dogechain.info/address/${address}`;
    case 'BCH':
      return `https://blockchair.com/bitcoin-cash/address/${address}`;
    case 'DGB':
      return `https://digiexplorer.info/address/${address}`;
    case 'SOL':
      return `https://solscan.io/account/${address}`;
    default:
      return `https://polygonscan.com/address/${address}`;
  }
}

export function explorerTxUrl(coin: string, txHash: string): string {
  switch (coin) {
    case 'BTC':
      return `https://mempool.space/tx/${txHash}`;
    case 'LTC':
      return `https://litecoinspace.org/tx/${txHash}`;
    case 'DOGE':
      return `https://dogechain.info/tx/${txHash}`;
    case 'BCH':
      return `https://blockchair.com/bitcoin-cash/transaction/${txHash}`;
    case 'DGB':
      return `https://digiexplorer.info/tx/${txHash}`;
    case 'SOL':
      return `https://solscan.io/tx/${txHash}`;
    default:
      return `https://polygonscan.com/tx/${txHash}`;
  }
}

export function shortHash(value: string, head = HASH_PREFIX_CHARS, tail = HASH_SUFFIX_CHARS): string {
  const ellipsisOverhead = 1;
  if (value.length <= head + tail + ellipsisOverhead) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

export function humanizeRpcError(error: string | null | undefined): string | null {
  if (!error) return null;
  const e = error.toLowerCase();
  if (e.includes('dgb') || e.includes('digibyte') || e.includes('insight')) return 'Explorer DGB offline';
  if (e.includes('timeout') || e.includes('timed out')) return 'RPC sem resposta';
  if (e.includes('error sending request') || e.includes('connect')) return 'Falha de rede no nó';
  if (e.includes('hot key ausente')) return 'Seed/chave de saque ausente';
  if (error.length > RPC_ERROR_PREVIEW_MAX_CHARS) {
    return `${error.slice(0, RPC_ERROR_PREVIEW_MAX_CHARS - RPC_ERROR_ELLIPSIS_CHARS)}…`;
  }
  return error;
}

export function withdrawalStatusLabel(status: string): string {
  return WITHDRAWAL_STATUS_LABELS[status] ?? status;
}

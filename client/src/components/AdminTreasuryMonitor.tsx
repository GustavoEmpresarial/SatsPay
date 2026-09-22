import { useMemo } from 'react';
import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COINS, formatAmount, safeBigInt, type Coin } from '@/shared';

interface TreasuryWallet {
  role: string;
  coin: string;
  onchain: string;
  ledger: string;
  error: string | null;
}

interface TreasuryResponse {
  wallets: TreasuryWallet[];
  bnbGasWei?: string | null;
  bnbGasError?: string | null;
}

/** Compact solvency strip for admin overview — full detail lives on /admin/stake. */
function formatBnbWei(wei: string): string {
  try {
    const v = BigInt(wei);
    const base = 10n ** 18n;
    const whole = v / base;
    const frac = (v % base).toString().padStart(18, '0').slice(0, 6);
    return `${whole}.${frac} BNB`;
  } catch {
    return `${wei} wei`;
  }
}

export function AdminTreasuryMonitor() {
  const { data, isLoading, isError, refetch, isFetching } = useQuery<TreasuryResponse>({
    queryKey: ['admin-treasury-wallets', 'hot'],
    queryFn: () => api<TreasuryResponse>('/admin/treasury-wallets?scope=hot'),
    refetchInterval: 30_000,
  });

  const rows = useMemo(() => {
    const hot = (data?.wallets ?? []).filter((w) => w.role === 'hot');
    return COINS.map((coin) => {
      const h = hot.find((w) => w.coin === coin);
      const onchain = h?.onchain ?? '0';
      const custody = h?.ledger ?? '0';
      const gap = safeBigInt(onchain) - safeBigInt(custody);
      const short = !h?.error && gap < 0n;
      const interesting =
        short || safeBigInt(onchain) > 0n || safeBigInt(custody) > 0n || !!h?.error;
      return { coin, onchain, custody, gap, short, error: h?.error ?? null, interesting };
    }).filter((r) => r.interesting);
  }, [data?.wallets]);

  const shortCount = rows.filter((r) => r.short).length;

  return (
    <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div>
          <h2 className="text-sm font-black text-ink">Cobertura de saque</h2>
          <p className="text-[11px] text-ink-muted">Saldo on-chain da hot − custódia dos usuários</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => refetch()}
            className="rounded-xl border border-border bg-surface px-2.5 py-1.5 text-xs font-bold text-ink"
          >
            <i className={`bi bi-arrow-repeat ${isFetching ? 'animate-spin' : ''}`} />
          </button>
          <Link
            to="/admin/stake"
            className="rounded-xl border border-border bg-surface px-2.5 py-1.5 text-xs font-bold text-ink hover:bg-paper"
          >
            Detalhe
          </Link>
        </div>
      </div>

      {shortCount > 0 && (
        <p className="text-xs font-bold text-rose-600">
          {shortCount} moeda(s) sem caixa suficiente na hot.
        </p>
      )}

      {(data?.bnbGasWei || data?.bnbGasError) && (
        <p className="text-[11px] text-ink-muted">
          Gas BNB (PEPE):{' '}
          <strong className="font-mono text-ink">
            {data.bnbGasWei ? formatBnbWei(data.bnbGasWei) : 'indisponível'}
          </strong>
          {data.bnbGasError ? ` — ${data.bnbGasError}` : ''}
        </p>
      )}

      {isLoading ? (
        <p className="py-4 text-center text-xs text-ink-muted animate-pulse">Consultando…</p>
      ) : isError ? (
        <p className="py-4 text-center text-xs text-rose-600">Falha ao ler carteiras hot</p>
      ) : rows.length === 0 ? (
        <p className="py-4 text-center text-xs text-ink-muted">Tudo zerado</p>
      ) : (
        <table className="w-full text-left text-xs">
          <thead className="border-b border-border text-[10px] uppercase tracking-wider text-ink-muted font-bold">
            <tr>
              <th className="py-2">Moeda</th>
              <th className="py-2 text-right">Hot</th>
              <th className="py-2 text-right">Custódia</th>
              <th className="py-2 text-right">Diferença</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border font-mono">
            {rows.map((r) => (
              <tr key={r.coin} className={r.short ? 'bg-rose-500/5' : undefined}>
                <td className="py-2 font-sans">
                  <div className="flex items-center gap-1.5 font-black text-ink">
                    <img src={coinLogo(r.coin)} alt="" className="h-4 w-4 rounded-full" />
                    {r.coin}
                    {r.error && (
                      <span className="text-[9px] font-bold text-amber-600 uppercase">RPC</span>
                    )}
                  </div>
                </td>
                <td className="py-2 text-right font-bold text-ink">
                  {formatAmount(r.onchain, r.coin as Coin)}
                </td>
                <td className="py-2 text-right text-ink-muted">
                  {formatAmount(r.custody, r.coin as Coin)}
                </td>
                <td
                  className={`py-2 text-right font-bold ${
                    r.gap < 0n ? 'text-rose-600' : 'text-emerald-600'
                  }`}
                >
                  {r.gap < 0n ? '−' : '+'}
                  {formatAmount(
                    (r.gap < 0n ? -r.gap : r.gap).toString(),
                    r.coin as Coin,
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

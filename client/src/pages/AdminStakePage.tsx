import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COINS, COIN_CONFIG, formatAmount, safeBigInt, type Coin } from '@/shared';

interface TreasuryWallet {
  role: 'hot' | 'deposit' | string;
  coin: string;
  address: string;
  hd_index: number | null;
  email: string | null;
  onchain: string;
  ledger: string;
  error: string | null;
}

interface TreasuryResponse {
  wallets: TreasuryWallet[];
  explorers: Record<string, string>;
}

interface EconCoinFlow {
  coin: string;
  count: number;
  volume: string;
  fees: string;
}

interface NetworkFeeByKind {
  coin: string;
  kind: string;
  count: number;
  amount: string;
}

interface FeeMarginByCoin {
  coin: string;
  fees_earned: string;
  network_paid: string;
  faucet_cost: string;
  fee_margin: string;
  operating_margin: string;
  healthy: boolean;
}

interface EconWindow {
  faucet_claims: number;
  faucet_by_coin: EconCoinFlow[];
  gateway_paid: number;
  gateway_created: number;
  gateway_by_coin: EconCoinFlow[];
  deposits_count: number;
  deposits_by_coin: EconCoinFlow[];
  withdrawals_count: number;
  withdrawals_by_coin: EconCoinFlow[];
  swap_count: number;
  swap_fees_by_coin: EconCoinFlow[];
  network_by_kind: NetworkFeeByKind[];
  fee_margin_by_coin: FeeMarginByCoin[];
}

interface PlatformEconomics {
  all_time: EconWindow;
  last_24h: EconWindow;
}

interface TreasuryHealth {
  pnl_usd: {
    fees_earned_usd: string;
    network_paid_usd: string;
    faucet_cost_usd: string;
    fee_margin_usd: string;
    operating_margin_usd: string;
    price_decimals: number;
    prices_available: boolean;
  };
  break_even: Array<{
    coin: string;
    withdrawal_fee: string;
    avg_network_fee: string;
    sample_count: number;
    covers: boolean;
    gap: string;
  }>;
  house_runway: Array<{
    coin: string;
    house_balance: string;
    burn_24h: string;
    burn_7d: string;
    days_at_24h_rate: number | null;
    days_at_7d_rate: number | null;
  }>;
  pending_liabilities: Array<{
    coin: string;
    count: number;
    amount: string;
    platform_fees: string;
    est_network_fees: string;
    total_out: string;
  }>;
  hot_buffers: Array<{
    coin: string;
    onchain: string;
    custody: string;
    pending_out: string;
    target: string;
    shortfall: string;
    status: string;
  }>;
  fee_series_7d: Array<{
    day: string;
    fees_earned: string;
    network_paid: string;
    fees_earned_usd: string;
    network_paid_usd: string;
  }>;
  fee_series_30d: Array<{
    day: string;
    fees_earned: string;
    network_paid: string;
    fees_earned_usd: string;
    network_paid_usd: string;
  }>;
  fee_margin_block: {
    enabled: boolean;
    blocked_coins: string[];
  };
}

type CoverStatus = 'ok' | 'short' | 'rpc';

function fmtUsdScaled(raw: string, decimals: number): string {
  const n = safeBigInt(raw);
  const neg = n < 0n;
  const abs = neg ? -n : n;
  const div = 10n ** BigInt(Math.max(0, decimals));
  const whole = abs / div;
  const frac = abs % div;
  const fracStr = frac.toString().padStart(decimals, '0').slice(0, 2).padEnd(2, '0');
  return `${neg ? '-' : ''}$${whole.toString()}.${fracStr}`;
}

function coverStatus(hot: string, custody: string, error: string | null | undefined): CoverStatus {
  if (error) return 'rpc';
  return safeBigInt(hot) >= safeBigInt(custody) ? 'ok' : 'short';
}

function addUnits(a: string, b: string): string {
  return (safeBigInt(a) + safeBigInt(b)).toString();
}

function flowMap(rows: EconCoinFlow[] | undefined): Map<string, EconCoinFlow> {
  const m = new Map<string, EconCoinFlow>();
  for (const r of rows ?? []) m.set(r.coin.toUpperCase(), r);
  return m;
}

function MiniStat({
  label,
  value,
  hint,
  tone = 'default',
}: {
  label: string;
  value: string | number;
  hint?: string;
  tone?: 'default' | 'good' | 'warn' | 'bad' | 'info';
}) {
  const cls =
    tone === 'good'
      ? 'text-emerald-600'
      : tone === 'warn'
        ? 'text-amber-600'
        : tone === 'bad'
          ? 'text-rose-600'
          : tone === 'info'
            ? 'text-sky-600'
            : 'text-ink';
  return (
    <div>
      <p className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">{label}</p>
      <p className={`mt-0.5 text-xl font-black font-mono tabular-nums ${cls}`}>{value}</p>
      {hint ? <p className="text-[10px] text-ink-muted mt-0.5">{hint}</p> : null}
    </div>
  );
}

export function AdminStakePage() {
  const [copied, setCopied] = useState<string | null>(null);

  const treasuryQ = useQuery<TreasuryResponse>({
    queryKey: ['admin-treasury-wallets'],
    queryFn: () => api<TreasuryResponse>('/admin/treasury-wallets'),
    refetchInterval: 30_000,
  });

  const econQ = useQuery<PlatformEconomics>({
    queryKey: ['admin-economics'],
    queryFn: () => api<PlatformEconomics>('/admin/economics'),
    refetchInterval: 30_000,
  });

  const healthQ = useQuery<TreasuryHealth>({
    queryKey: ['admin-treasury-health'],
    queryFn: () => api<TreasuryHealth>('/admin/treasury-health'),
    refetchInterval: 30_000,
  });

  const wallets = treasuryQ.data?.wallets ?? [];
  const explorers = treasuryQ.data?.explorers ?? {};
  const all = econQ.data?.all_time;
  const h24 = econQ.data?.last_24h;
  const health = healthQ.data;

  const hotByCoin = useMemo(() => {
    const map = new Map<string, TreasuryWallet>();
    for (const w of wallets) {
      if (w.role === 'hot') map.set(w.coin, w);
    }
    return map;
  }, [wallets]);

  const userDep = useMemo(() => flowMap(all?.deposits_by_coin), [all]);
  const gatewayDep = useMemo(() => flowMap(all?.gateway_by_coin), [all]);
  const withdrawals = useMemo(() => flowMap(all?.withdrawals_by_coin), [all]);

  const rows = useMemo(() => {
    return COINS.map((coin) => {
      const hot = hotByCoin.get(coin);
      const onchain = hot?.onchain ?? '0';
      const custody = hot?.ledger ?? '0';
      const userVol = userDep.get(coin)?.volume ?? '0';
      const gwVol = gatewayDep.get(coin)?.volume ?? '0';
      const depTotal = addUnits(userVol, gwVol);
      const depCount = (userDep.get(coin)?.count ?? 0) + (gatewayDep.get(coin)?.count ?? 0);
      const wd = withdrawals.get(coin);
      const status = coverStatus(onchain, custody, hot?.error);
      const gap = safeBigInt(onchain) - safeBigInt(custody);
      return {
        coin,
        onchain,
        custody,
        depTotal,
        depCount,
        userVol,
        gwVol,
        wdVol: wd?.volume ?? '0',
        wdCount: wd?.count ?? 0,
        status,
        gap,
        address: hot?.address,
        error: hot?.error,
      };
    });
  }, [hotByCoin, userDep, gatewayDep, withdrawals]);

  const shortCount = rows.filter((r) => r.status === 'short').length;
  const okCount = rows.filter((r) => r.status === 'ok').length;
  const rpcCount = rows.filter((r) => r.status === 'rpc').length;
  const coverMax = Math.max(1, okCount + shortCount + rpcCount);
  const solvencyPct = Math.round((okCount / coverMax) * 100);

  const coinsWithCustody = rows.filter((r) => safeBigInt(r.custody) > 0n);
  const coveredCustody = coinsWithCustody.filter((r) => r.status === 'ok').length;
  const custodyCoverPct =
    coinsWithCustody.length === 0
      ? 100
      : Math.round((coveredCustody / coinsWithCustody.length) * 100);

  const gatewayFeeRows = (all?.gateway_by_coin ?? []).filter((r) => safeBigInt(r.fees) > 0n);
  const wdFeeRows = (all?.withdrawals_by_coin ?? []).filter((r) => safeBigInt(r.fees) > 0n);
  const swapFeeRows = (all?.swap_fees_by_coin ?? []).filter((r) => safeBigInt(r.fees) > 0n);
  const faucetCostRows = (all?.faucet_by_coin ?? []).filter((r) => safeBigInt(r.volume) > 0n);
  const feeMargins = all?.fee_margin_by_coin ?? [];
  const networkByKind = all?.network_by_kind ?? [];
  const unhealthyMargins = feeMargins.filter((m) => !m.healthy).length;

  const unsweptByCoin = useMemo(() => {
    const m = new Map<string, { count: number; total: bigint }>();
    for (const w of wallets) {
      if (w.role !== 'deposit') continue;
      const bal = safeBigInt(w.onchain);
      if (bal <= 0n) continue;
      const cur = m.get(w.coin) ?? { count: 0, total: 0n };
      cur.count += 1;
      cur.total += bal;
      m.set(w.coin, cur);
    }
    return [...m.entries()].map(([coin, v]) => ({
      coin,
      count: v.count,
      total: v.total.toString(),
    }));
  }, [wallets]);

  const series = health?.fee_series_7d?.length ? health.fee_series_7d : health?.fee_series_30d ?? [];
  const seriesMaxUsd = useMemo(() => {
    let m = 1n;
    for (const d of series) {
      const a = safeBigInt(d.fees_earned_usd);
      const b = safeBigInt(d.network_paid_usd);
      if (a > m) m = a;
      if (b > m) m = b;
    }
    return m;
  }, [series]);

  const pd = health?.pnl_usd.price_decimals ?? 8;

  const flowBars = useMemo(() => {
    return rows
      .map((r) => ({
        coin: r.coin,
        in: safeBigInt(r.depTotal),
        out: safeBigInt(r.wdVol),
      }))
      .filter((r) => r.in > 0n || r.out > 0n)
      .slice(0, 9);
  }, [rows]);

  const flowMax = useMemo(() => {
    let m = 1n;
    for (const f of flowBars) {
      if (f.in > m) m = f.in;
      if (f.out > m) m = f.out;
    }
    return m;
  }, [flowBars]);

  const hotBars = useMemo(() => {
    return rows
      .filter((r) => safeBigInt(r.onchain) > 0n || safeBigInt(r.custody) > 0n)
      .slice(0, 8)
      .map((r) => ({
        coin: r.coin,
        hotRaw: safeBigInt(r.onchain),
        custRaw: safeBigInt(r.custody),
      }));
  }, [rows]);

  const hotBarMax = useMemo(() => {
    let m = 1n;
    for (const h of hotBars) {
      if (h.hotRaw > m) m = h.hotRaw;
      if (h.custRaw > m) m = h.custRaw;
    }
    return m;
  }, [hotBars]);

  const activityBars = [
    { label: 'Depósitos user 24h', value: h24?.deposits_count ?? 0, color: 'bg-emerald-500' },
    { label: 'Gateway pago 24h', value: h24?.gateway_paid ?? 0, color: 'bg-bitcoin' },
    { label: 'Saques 24h', value: h24?.withdrawals_count ?? 0, color: 'bg-sky-500' },
    { label: 'Claims de faucet 24h', value: h24?.faucet_claims ?? 0, color: 'bg-rose-400' },
  ];
  const activityMax = Math.max(1, ...activityBars.map((b) => b.value));

  const isLoading = treasuryQ.isLoading || econQ.isLoading || healthQ.isLoading;
  const isFetching = treasuryQ.isFetching || econQ.isFetching || healthQ.isFetching;
  const isError = treasuryQ.isError || econQ.isError || healthQ.isError;

  const copy = (text: string) => {
    if (!text) return;
    void navigator.clipboard.writeText(text);
    setCopied(text);
    setTimeout(() => setCopied(null), 1600);
  };

  const refresh = () => {
    void treasuryQ.refetch();
    void econQ.refetch();
    void healthQ.refetch();
  };

  return (
    <div className="space-y-6 pb-16 max-w-6xl mx-auto">
      <div className="flex flex-col sm:flex-row sm:items-end sm:justify-between gap-3">
        <div>
          <h1 className="text-2xl font-black text-ink tracking-tight">Tesouraria</h1>
          <p className="text-xs text-ink-muted mt-0.5">
            Nossa hot · custódia dos usuários · depósitos (carteira + gateway) · saques processados.
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          className="inline-flex items-center gap-2 self-start rounded-xl border border-border bg-paper px-3 py-2 text-xs font-bold text-ink hover:bg-surface"
        >
          <i className={`bi bi-arrow-repeat ${isFetching ? 'animate-spin' : ''}`} />
          Atualizar
        </button>
      </div>

      {shortCount > 0 && (
        <div className="rounded-xl border border-rose-500/40 bg-rose-500/10 px-4 py-3 text-xs font-bold text-rose-700">
          {shortCount} moeda(s) com hot abaixo da custódia — saques podem falhar.
        </div>
      )}

      {isError && (
        <div className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-xs font-bold text-rose-700">
          Falha ao carregar tesouraria.
        </div>
      )}

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <p className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">Nossa carteira</p>
          <p className="mt-1 text-sm font-black text-ink">Saldo on-chain</p>
          <p className="mt-1 text-[11px] text-ink-muted">Saldo nas hots da plataforma</p>
        </div>
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <p className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">Usuários</p>
          <p className="mt-1 text-sm font-black text-ink">Custódia (ledger)</p>
          <p className="mt-1 text-[11px] text-ink-muted">O que os usuários têm creditado</p>
        </div>
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <p className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">Entrada</p>
          <p className="mt-1 text-sm font-black text-emerald-700">Depósitos</p>
          <p className="mt-1 text-[11px] text-ink-muted">On-chain do usuário + gateway pago</p>
        </div>
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <p className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">Saída</p>
          <p className="mt-1 text-sm font-black text-sky-700">Saques</p>
          <p className="mt-1 text-[11px] text-ink-muted">Transmitidos / confirmados</p>
        </div>
      </div>

      <div className="rounded-2xl border border-border bg-paper overflow-hidden shadow-xs">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="border-b border-border text-[10px] uppercase tracking-wider text-ink-muted font-bold bg-surface/60">
              <tr>
                <th className="py-2.5 px-3">Moeda</th>
                <th className="py-2.5 px-3 text-right">Nossa (hot)</th>
                <th className="py-2.5 px-3 text-right">Usuários</th>
                <th className="py-2.5 px-3 text-right">Depósitos</th>
                <th className="py-2.5 px-3 text-right">Saques</th>
                <th className="py-2.5 px-3">Status</th>
                <th className="py-2.5 px-3">Endereço hot</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border font-mono">
              {isLoading
                ? COINS.map((c) => (
                    <tr key={c}>
                      <td colSpan={7} className="py-4 px-3 animate-pulse text-ink-muted font-sans">
                        …
                      </td>
                    </tr>
                  ))
                : rows.map((r) => {
                    const explorer =
                      explorers[r.coin] && r.address ? `${explorers[r.coin]}${r.address}` : null;
                    const rowTone =
                      r.status === 'short' ? 'bg-rose-500/5' : r.status === 'rpc' ? 'bg-amber-500/5' : '';

                    return (
                      <tr key={r.coin} className={rowTone}>
                        <td className="py-3 px-3 font-sans">
                          <div className="flex items-center gap-2">
                            <img src={coinLogo(r.coin)} alt="" className="h-5 w-5 rounded-full" />
                            <div>
                              <div className="font-black text-ink leading-none">{r.coin}</div>
                              <div className="text-[10px] text-ink-muted font-bold mt-0.5">
                                {COIN_CONFIG[r.coin].name}
                              </div>
                            </div>
                          </div>
                        </td>
                        <td className="py-3 px-3 text-right font-bold text-ink">
                          {formatAmount(r.onchain, r.coin)}
                        </td>
                        <td className="py-3 px-3 text-right text-ink-muted">
                          {formatAmount(r.custody, r.coin)}
                        </td>
                        <td className="py-3 px-3 text-right">
                          <div className="font-bold text-emerald-700">
                            {formatAmount(r.depTotal, r.coin)}
                          </div>
                          <div className="text-[10px] text-ink-muted font-sans">
                            {r.depCount > 0
                              ? `${r.depCount}× · user ${formatAmount(r.userVol, r.coin)} + gw ${formatAmount(r.gwVol, r.coin)}`
                              : '—'}
                          </div>
                        </td>
                        <td className="py-3 px-3 text-right">
                          <div className="font-bold text-sky-700">{formatAmount(r.wdVol, r.coin)}</div>
                          <div className="text-[10px] text-ink-muted font-sans">
                            {r.wdCount > 0 ? `${r.wdCount}×` : '—'}
                          </div>
                        </td>
                        <td className="py-3 px-3 font-sans">
                          {r.status === 'ok' && (
                            <span className="text-[10px] font-black uppercase text-emerald-600">Ok</span>
                          )}
                          {r.status === 'short' && (
                            <span className="text-[10px] font-black uppercase text-rose-600">Falta caixa</span>
                          )}
                          {r.status === 'rpc' && (
                            <span
                              className="text-[10px] font-black uppercase text-amber-600"
                              title={r.error ?? ''}
                            >
                              RPC
                            </span>
                          )}
                          {r.gap !== 0n && r.status !== 'rpc' && (
                            <div
                              className={`text-[10px] font-mono mt-0.5 ${
                                r.gap < 0n ? 'text-rose-600' : 'text-emerald-600'
                              }`}
                            >
                              diferença {r.gap < 0n ? '−' : '+'}
                              {formatAmount((r.gap < 0n ? -r.gap : r.gap).toString(), r.coin)}
                            </div>
                          )}
                        </td>
                        <td className="py-3 px-3 font-sans">
                          {r.address ? (
                            <div className="flex items-center gap-1.5 max-w-[220px]">
                              <span className="truncate font-mono text-[11px] text-ink" title={r.address}>
                                {r.address}
                              </span>
                              <button
                                type="button"
                                className="shrink-0 text-ink-muted hover:text-ink"
                                onClick={() => copy(r.address!)}
                                title="Copiar"
                              >
                                <i
                                  className={`bi ${copied === r.address ? 'bi-check text-emerald-600' : 'bi-copy'}`}
                                />
                              </button>
                              {explorer && (
                                <a
                                  href={explorer}
                                  target="_blank"
                                  rel="noreferrer"
                                  className="shrink-0 text-bitcoin hover:opacity-80"
                                  title="Explorer"
                                >
                                  <i className="bi bi-box-arrow-up-right" />
                                </a>
                              )}
                            </div>
                          ) : (
                            <span className="text-ink-muted italic">—</span>
                          )}
                        </td>
                      </tr>
                    );
                  })}
            </tbody>
          </table>
        </div>
      </div>

      <p className="text-[11px] text-ink-muted px-1">
        Depósitos = créditos de carteira pessoal + volume de faturas do gateway confirmadas. Saques = status
        transmitidos/confirmados. Histórico completo.
      </p>

      <div>
        <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
          <h2 className="text-sm font-black text-ink">Saúde financeira</h2>
          <Link to="/admin" className="text-[11px] font-bold text-bitcoin hover:underline">
            Economia completa →
          </Link>
        </div>

        <div className="grid md:grid-cols-2 xl:grid-cols-3 gap-3">
          {/* Solvência */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-700">
                <i className="bi bi-safe2" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Solvência</h3>
                <p className="text-[11px] text-ink-muted">A hot cobre a custódia?</p>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <MiniStat
                label="Cobertura"
                value={`${solvencyPct}%`}
                hint={`${okCount}/${coverMax} moedas ok`}
                tone={shortCount > 0 ? 'bad' : 'good'}
              />
              <MiniStat
                label="Com custódia"
                value={`${custodyCoverPct}%`}
                hint={
                  coinsWithCustody.length
                    ? `${coveredCustody}/${coinsWithCustody.length} cobertas`
                    : 'sem custódia'
                }
                tone={custodyCoverPct < 100 && coinsWithCustody.length > 0 ? 'warn' : 'good'}
              />
            </div>
            <div className="flex h-2.5 rounded-full overflow-hidden bg-border">
              <div className="bg-emerald-500" style={{ width: `${(okCount / coverMax) * 100}%` }} />
              <div className="bg-rose-500" style={{ width: `${(shortCount / coverMax) * 100}%` }} />
              <div className="bg-amber-500" style={{ width: `${(rpcCount / coverMax) * 100}%` }} />
            </div>
            <div className="flex justify-between text-[10px] font-bold text-ink-muted">
              <span className="text-emerald-600">Ok {okCount}</span>
              <span className="text-rose-600">Falta {shortCount}</span>
              <span className="text-amber-600">RPC {rpcCount}</span>
            </div>
          </div>

          {/* Fluxo all-time */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin">
                <i className="bi bi-arrow-left-right" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Fluxo (histórico)</h3>
                <p className="text-[11px] text-ink-muted">Contagens processadas</p>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <MiniStat
                label="Depósitos user"
                value={all?.deposits_count ?? 0}
                tone="good"
              />
              <MiniStat label="Gateway pago" value={all?.gateway_paid ?? 0} tone="good" />
              <MiniStat label="Saques" value={all?.withdrawals_count ?? 0} tone="info" />
              <MiniStat
                label="Faucet (custo)"
                value={all?.faucet_claims ?? 0}
                tone="bad"
              />
            </div>
          </div>

          {/* Receita vs custo */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-700">
                <i className="bi bi-cash-stack" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Receita & custo</h3>
                <p className="text-[11px] text-ink-muted">Taxas retidas · faucet HOUSE</p>
              </div>
            </div>
            <div className="space-y-2 text-xs">
              <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 px-3 py-2">
                <div className="text-[10px] font-bold uppercase text-emerald-800">Receita gateway</div>
                {gatewayFeeRows.length === 0 ? (
                  <p className="mt-1 text-ink-muted">—</p>
                ) : (
                  <ul className="mt-1 space-y-0.5">
                    {gatewayFeeRows.map((r) => (
                      <li key={r.coin} className="flex justify-between font-mono font-bold text-emerald-700">
                        <span>{r.coin}</span>
                        <span>+{isCoinFmt(r.fees, r.coin)}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="rounded-xl border border-sky-500/20 bg-sky-500/5 px-3 py-2">
                <div className="text-[10px] font-bold uppercase text-sky-800">Taxas de saque</div>
                {wdFeeRows.length === 0 ? (
                  <p className="mt-1 text-ink-muted">—</p>
                ) : (
                  <ul className="mt-1 space-y-0.5">
                    {wdFeeRows.map((r) => (
                      <li key={r.coin} className="flex justify-between font-mono font-bold text-sky-700">
                        <span>{r.coin}</span>
                        <span>+{isCoinFmt(r.fees, r.coin)}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="rounded-xl border border-rose-500/20 bg-rose-500/5 px-3 py-2">
                <div className="text-[10px] font-bold uppercase text-rose-800">Custo faucet</div>
                {faucetCostRows.length === 0 ? (
                  <p className="mt-1 text-ink-muted">—</p>
                ) : (
                  <ul className="mt-1 space-y-0.5">
                    {faucetCostRows.map((r) => (
                      <li key={r.coin} className="flex justify-between font-mono font-bold text-rose-700">
                        <span>{r.coin}</span>
                        <span>−{isCoinFmt(r.volume, r.coin)}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              {swapFeeRows.length > 0 && (
                <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 px-3 py-2">
                  <div className="text-[10px] font-bold uppercase text-amber-800">Taxas swap</div>
                  <ul className="mt-1 space-y-0.5">
                    {swapFeeRows.map((r) => (
                      <li key={r.coin} className="flex justify-between font-mono font-bold text-amber-800">
                        <span>{r.coin}</span>
                        <span>+{isCoinFmt(r.fees, r.coin)}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          </div>

          {/* Margem taxas vs rede */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-start justify-between gap-2">
              <div className="flex items-center gap-2">
                <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin">
                  <i className="bi bi-graph-up-arrow" />
                </div>
                <div>
                  <h3 className="text-sm font-black text-ink">Margem de taxas</h3>
                  <p className="text-[11px] text-ink-muted">Cobradas − rede (histórico)</p>
                </div>
              </div>
              {feeMargins.length > 0 &&
                (unhealthyMargins > 0 ? (
                  <span className="rounded-full border border-rose-500/30 bg-rose-500/10 px-2 py-0.5 text-[10px] font-black uppercase text-rose-700">
                    {unhealthyMargins} no vermelho
                  </span>
                ) : (
                  <span className="rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2 py-0.5 text-[10px] font-black uppercase text-emerald-700">
                    Ok
                  </span>
                ))}
            </div>
            <p className="text-[10px] text-ink-muted">Rede = saques / sweeps / DEX gravados após o deploy.</p>
            {feeMargins.length === 0 ? (
              <p className="text-xs text-ink-muted">—</p>
            ) : (
              <ul className="space-y-1.5 text-xs">
                {feeMargins.map((m) => (
                  <li
                    key={m.coin}
                    className={`rounded-xl border px-3 py-2 ${
                      m.healthy ? 'border-emerald-500/20 bg-emerald-500/5' : 'border-rose-500/20 bg-rose-500/5'
                    }`}
                  >
                    <div className="flex justify-between font-bold">
                      <span>{m.coin}</span>
                      <span className={`font-mono ${m.healthy ? 'text-emerald-700' : 'text-rose-700'}`}>
                        {isCoinFmt(m.fee_margin, m.coin)}
                      </span>
                    </div>
                    <div className="mt-0.5 flex justify-between font-mono text-[10px] text-ink-muted">
                      <span>+{isCoinFmt(m.fees_earned, m.coin)} taxas</span>
                      <span>−{isCoinFmt(m.network_paid, m.coin)} rede</span>
                    </div>
                  </li>
                ))}
              </ul>
            )}
            {networkByKind.length > 0 && (
              <div className="space-y-1 border-t border-border pt-2">
                <div className="text-[10px] font-bold uppercase text-ink-muted">Rede por tipo</div>
                {networkByKind.map((n) => (
                  <div key={`${n.coin}-${n.kind}`} className="flex justify-between text-[11px] font-mono">
                    <span className="text-ink-muted">
                      {n.coin} {networkKindLabel(n.kind)} ×{n.count}
                    </span>
                    <span className="font-bold text-rose-700">{isCoinFmt(n.amount, n.coin)}</span>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Atividade 24h */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div>
              <h3 className="text-sm font-black text-ink">Atividade 24h</h3>
              <p className="text-[11px] text-ink-muted">Ops financeiras recentes</p>
            </div>
            <div className="space-y-2">
              {activityBars.map((b) => (
                <div key={b.label}>
                  <div className="flex justify-between text-[11px] font-bold mb-0.5">
                    <span className="text-ink-muted">{b.label}</span>
                    <span className="font-mono text-ink">{b.value}</span>
                  </div>
                  <div className="h-1.5 rounded-full bg-border overflow-hidden">
                    <div
                      className={`h-full rounded-full ${b.color}`}
                      style={{ width: `${(b.value / activityMax) * 100}%` }}
                    />
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* In vs Out chart */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3 md:col-span-1 xl:col-span-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <h3 className="text-sm font-black text-ink">Depósitos vs saques</h3>
                <p className="text-[11px] text-ink-muted">Volume relativo por moeda (histórico)</p>
              </div>
              <div className="flex items-center gap-3 text-[10px] font-bold text-ink-muted">
                <span className="inline-flex items-center gap-1">
                  <span className="h-2 w-2 rounded-sm bg-emerald-500" /> Entrada
                </span>
                <span className="inline-flex items-center gap-1">
                  <span className="h-2 w-2 rounded-sm bg-sky-500" /> Saída
                </span>
              </div>
            </div>
            {flowBars.length === 0 ? (
              <p className="py-8 text-center text-xs text-ink-muted">Sem fluxo ainda.</p>
            ) : (
              <div className="flex items-end gap-3 sm:gap-4 min-h-[100px] pt-2">
                {flowBars.map((f) => {
                  const inH = Number((f.in * 100n) / flowMax);
                  const outH = Number((f.out * 100n) / flowMax);
                  return (
                    <div key={f.coin} className="flex-1 min-w-0 flex flex-col items-center gap-1">
                      <div className="flex items-end gap-0.5 h-20 w-full justify-center">
                        <div
                          className="w-2 sm:w-3 rounded-t bg-emerald-500"
                          style={{ height: `${Math.max(f.in > 0n ? 6 : 0, inH)}%` }}
                        />
                        <div
                          className="w-2 sm:w-3 rounded-t bg-sky-500"
                          style={{ height: `${Math.max(f.out > 0n ? 6 : 0, outH)}%` }}
                        />
                      </div>
                      <div className="flex items-center gap-1">
                        <img src={coinLogo(f.coin as Coin)} alt="" className="h-3.5 w-3.5 rounded-full" />
                        <span className="text-[10px] font-black text-ink">{f.coin}</span>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* Hot vs custody */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3 md:col-span-2 xl:col-span-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <h3 className="text-sm font-black text-ink">Hot × custódia</h3>
                <p className="text-[11px] text-ink-muted">
                  Nossa caixa on-chain vs o que os usuários têm no ledger.
                </p>
              </div>
              <div className="flex items-center gap-3 text-[10px] font-bold text-ink-muted">
                <span className="inline-flex items-center gap-1">
                  <span className="h-2 w-2 rounded-sm bg-bitcoin" /> Nossa hot
                </span>
                <span className="inline-flex items-center gap-1">
                  <span className="h-2 w-2 rounded-sm bg-ink/30" /> Usuários
                </span>
              </div>
            </div>
            {hotBars.length === 0 ? (
              <p className="py-8 text-center text-xs text-ink-muted">Sem saldo hot/custódia pra plotar.</p>
            ) : (
              <div className="flex items-end gap-3 sm:gap-4 min-h-[100px] pt-2">
                {hotBars.map((h) => {
                  const hotH = Number((h.hotRaw * 100n) / hotBarMax);
                  const custH = Number((h.custRaw * 100n) / hotBarMax);
                  return (
                    <div key={h.coin} className="flex-1 min-w-0 flex flex-col items-center gap-1">
                      <div className="flex items-end gap-0.5 h-20 w-full justify-center">
                        <div
                          className="w-2 sm:w-3 rounded-t bg-bitcoin"
                          style={{ height: `${Math.max(h.hotRaw > 0n ? 6 : 0, hotH)}%` }}
                        />
                        <div
                          className="w-2 sm:w-3 rounded-t bg-ink/25"
                          style={{ height: `${Math.max(h.custRaw > 0n ? 6 : 0, custH)}%` }}
                        />
                      </div>
                      <div className="flex items-center gap-1">
                        <img src={coinLogo(h.coin as Coin)} alt="" className="h-3.5 w-3.5 rounded-full" />
                        <span className="text-[10px] font-black text-ink">{h.coin}</span>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Operacional: 9 painéis */}
        <div className="mt-4 grid md:grid-cols-2 xl:grid-cols-3 gap-3">
          {/* 1. P&L USD */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-700">
                <i className="bi bi-currency-dollar" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Resultado (USD)</h3>
                <p className="text-[11px] text-ink-muted">Taxas cobradas − custo de rede − faucet</p>
              </div>
            </div>
            {!health?.pnl_usd.prices_available ? (
              <p className="text-xs text-ink-muted">Sem cotações — aguarde o worker de preços.</p>
            ) : (
              <div className="space-y-2 text-xs">
                <div className="flex justify-between font-mono">
                  <span className="text-ink-muted">Taxas +</span>
                  <span className="font-bold text-emerald-700">
                    {fmtUsdScaled(health.pnl_usd.fees_earned_usd, pd)}
                  </span>
                </div>
                <div className="flex justify-between font-mono">
                  <span className="text-ink-muted">Rede −</span>
                  <span className="font-bold text-rose-700">
                    {fmtUsdScaled(health.pnl_usd.network_paid_usd, pd)}
                  </span>
                </div>
                <div className="flex justify-between font-mono border-t border-border pt-2">
                  <span className="font-bold">Margem taxas</span>
                  <span
                    className={`font-black ${
                      safeBigInt(health.pnl_usd.fee_margin_usd) < 0n ? 'text-rose-700' : 'text-emerald-700'
                    }`}
                  >
                    {fmtUsdScaled(health.pnl_usd.fee_margin_usd, pd)}
                  </span>
                </div>
                <div className="flex justify-between font-mono">
                  <span className="text-ink-muted">Faucet −</span>
                  <span className="font-bold text-rose-600">
                    {fmtUsdScaled(health.pnl_usd.faucet_cost_usd, pd)}
                  </span>
                </div>
                <div className="flex justify-between font-mono">
                  <span className="font-bold">Resultado operacional</span>
                  <span
                    className={`font-black ${
                      safeBigInt(health.pnl_usd.operating_margin_usd) < 0n
                        ? 'text-rose-700'
                        : 'text-emerald-700'
                    }`}
                  >
                    {fmtUsdScaled(health.pnl_usd.operating_margin_usd, pd)}
                  </span>
                </div>
              </div>
            )}
          </div>

          {/* 2. Break-even */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-amber-500/10 text-amber-700">
                <i className="bi bi-scales" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Equilíbrio do saque</h3>
                <p className="text-[11px] text-ink-muted">Taxa cobrada × média de rede</p>
              </div>
            </div>
            <ul className="max-h-48 overflow-y-auto space-y-1 text-xs">
              {(health?.break_even ?? []).map((b) => (
                <li
                  key={b.coin}
                  className={`flex justify-between gap-2 rounded-lg px-2 py-1.5 ${
                    b.covers ? 'bg-emerald-500/5' : 'bg-rose-500/10'
                  }`}
                >
                  <span className="font-bold">
                    {b.coin}
                    {b.sample_count === 0 ? (
                      <span className="ml-1 text-[10px] font-medium text-ink-muted">sem histórico</span>
                    ) : null}
                  </span>
                  <span className={`font-mono font-bold ${b.covers ? 'text-emerald-700' : 'text-rose-700'}`}>
                    {b.covers ? 'Ok' : 'taxa baixa'}
                  </span>
                </li>
              ))}
            </ul>
          </div>

          {/* 3. HOUSE runway */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-rose-500/10 text-rose-700">
                <i className="bi bi-droplet-half" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Autonomia da HOUSE</h3>
                <p className="text-[11px] text-ink-muted">Dias restantes de faucet</p>
              </div>
            </div>
            {(health?.house_runway ?? []).length === 0 ? (
              <p className="text-xs text-ink-muted">Sem inventário nem consumo de faucet.</p>
            ) : (
              <ul className="space-y-1.5 text-xs">
                {health!.house_runway.map((r) => (
                  <li key={r.coin} className="rounded-lg border border-border px-2 py-1.5">
                    <div className="flex justify-between font-bold">
                      <span>{r.coin}</span>
                      <span className="font-mono">
                        {r.days_at_24h_rate != null
                          ? `${r.days_at_24h_rate.toFixed(1)}d @24h`
                          : r.days_at_7d_rate != null
                            ? `${r.days_at_7d_rate.toFixed(1)}d @7d`
                            : '∞'}
                      </span>
                    </div>
                    <div className="text-[10px] text-ink-muted font-mono">
                      bal {isCoinFmt(r.house_balance, r.coin)} · burn24h {isCoinFmt(r.burn_24h, r.coin)}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {/* 4. Passivos pendentes */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-sky-500/10 text-sky-700">
                <i className="bi bi-hourglass-split" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Passivos pendentes</h3>
                <p className="text-[11px] text-ink-muted">Saques ainda na fila</p>
              </div>
            </div>
            {(health?.pending_liabilities ?? []).length === 0 ? (
              <p className="text-xs text-ink-muted">Nenhum saque pendente.</p>
            ) : (
              <ul className="space-y-1.5 text-xs">
                {health!.pending_liabilities.map((p) => (
                  <li key={p.coin} className="rounded-lg border border-border px-2 py-1.5">
                    <div className="flex justify-between font-bold">
                      <span>
                        {p.coin} ×{p.count}
                      </span>
                      <span className="font-mono text-sky-700">{isCoinFmt(p.total_out, p.coin)}</span>
                    </div>
                    <div className="text-[10px] text-ink-muted font-mono">
                      amt {isCoinFmt(p.amount, p.coin)} + rede≈{isCoinFmt(p.est_network_fees, p.coin)}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {/* 5. Sweeps parados */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin">
                <i className="bi bi-arrow-repeat" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Varreduras pendentes</h3>
                <p className="text-[11px] text-ink-muted">Saldo em depósitos ainda não varridos</p>
              </div>
            </div>
            {unsweptByCoin.length === 0 ? (
              <p className="text-xs text-ink-muted">Nada parado nos endereços de depósito.</p>
            ) : (
              <ul className="space-y-1.5 text-xs">
                {unsweptByCoin.map((u) => (
                  <li key={u.coin} className="flex justify-between font-mono font-bold">
                    <span>
                      {u.coin} · {u.count} addr
                    </span>
                    <span className="text-amber-700">{isCoinFmt(u.total, u.coin)}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {/* 6. Buffer mínimo hot */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-ink/5 text-ink">
                <i className="bi bi-battery-half" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Reserva da hot</h3>
                <p className="text-[11px] text-ink-muted">Meta = pendente + 20× taxa de saque</p>
              </div>
            </div>
            <ul className="max-h-52 overflow-y-auto space-y-1 text-xs">
              {(health?.hot_buffers ?? [])
                .filter((b) => b.status !== 'ok' || safeBigInt(b.onchain) > 0n || safeBigInt(b.custody) > 0n)
                .map((b) => (
                  <li
                    key={b.coin}
                    className={`rounded-lg px-2 py-1.5 ${
                      b.status === 'critical'
                        ? 'bg-rose-500/10'
                        : b.status === 'low'
                          ? 'bg-amber-500/10'
                          : b.status === 'rpc'
                            ? 'bg-amber-500/5'
                            : 'bg-surface/60'
                    }`}
                  >
                    <div className="flex justify-between font-bold">
                      <span>{b.coin}</span>
                      <span className="uppercase text-[10px] tracking-wide">{bufferStatusLabel(b.status)}</span>
                    </div>
                    <div className="font-mono text-[10px] text-ink-muted">
                      hot {isCoinFmt(b.onchain, b.coin)} / meta {isCoinFmt(b.target, b.coin)}
                      {safeBigInt(b.shortfall) > 0n ? ` · faltam ${isCoinFmt(b.shortfall, b.coin)}` : ''}
                    </div>
                  </li>
                ))}
            </ul>
          </div>

          {/* 7. Trava operacional */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-rose-500/10 text-rose-700">
                <i className="bi bi-shield-lock" />
              </div>
              <div>
                <h3 className="text-sm font-black text-ink">Trava de margem</h3>
                <p className="text-[11px] text-ink-muted">Bloqueia saque e faucet se a rede custar mais que as taxas</p>
              </div>
            </div>
            {health?.fee_margin_block.enabled ? (
              (health.fee_margin_block.blocked_coins.length === 0 ? (
                <p className="text-xs font-bold text-emerald-700">Ativa · nenhuma moeda bloqueada</p>
              ) : (
                <div className="space-y-1">
                  <p className="text-xs font-bold text-rose-700">Moedas bloqueadas:</p>
                  <div className="flex flex-wrap gap-1">
                    {health.fee_margin_block.blocked_coins.map((c) => (
                      <span
                        key={c}
                        className="rounded-full border border-rose-500/30 bg-rose-500/10 px-2 py-0.5 text-[10px] font-black text-rose-700"
                      >
                        {c}
                      </span>
                    ))}
                  </div>
                </div>
              ))
            ) : (
              <p className="text-xs text-amber-700 font-bold">Desligada (variável FEE_MARGIN_HARD_BLOCK=0)</p>
            )}
          </div>

          {/* 8. Série 7d */}
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3 md:col-span-2 xl:col-span-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <h3 className="text-sm font-black text-ink">Taxas × rede (7 dias)</h3>
                <p className="text-[11px] text-ink-muted">USD diário pela cotação em cache</p>
              </div>
              <div className="flex items-center gap-3 text-[10px] font-bold text-ink-muted">
                <span className="inline-flex items-center gap-1">
                  <span className="h-2 w-2 rounded-sm bg-emerald-500" /> Taxas
                </span>
                <span className="inline-flex items-center gap-1">
                  <span className="h-2 w-2 rounded-sm bg-rose-500" /> Rede
                </span>
              </div>
            </div>
            {series.length === 0 ? (
              <p className="py-6 text-center text-xs text-ink-muted">Sem série ainda (sem taxas nem rede no período).</p>
            ) : (
              <div className="flex items-end gap-1.5 sm:gap-2 min-h-[100px] pt-2 overflow-x-auto">
                {series.map((d) => {
                  const eH = Number((safeBigInt(d.fees_earned_usd) * 100n) / seriesMaxUsd);
                  const nH = Number((safeBigInt(d.network_paid_usd) * 100n) / seriesMaxUsd);
                  return (
                    <div key={d.day} className="flex-1 min-w-[28px] flex flex-col items-center gap-1">
                      <div className="flex items-end gap-0.5 h-20 w-full justify-center">
                        <div
                          className="w-1.5 sm:w-2 rounded-t bg-emerald-500"
                          style={{
                            height: `${Math.max(safeBigInt(d.fees_earned_usd) > 0n ? 4 : 0, eH)}%`,
                          }}
                        />
                        <div
                          className="w-1.5 sm:w-2 rounded-t bg-rose-500"
                          style={{
                            height: `${Math.max(safeBigInt(d.network_paid_usd) > 0n ? 4 : 0, nH)}%`,
                          }}
                        />
                      </div>
                      <span className="text-[9px] font-bold text-ink-muted">
                        {d.day.slice(5)}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
            {(health?.fee_series_30d?.length ?? 0) > 0 && (
              <p className="text-[10px] text-ink-muted">
                30d: {health!.fee_series_30d.length} dias com movimento · última{' '}
                {health!.fee_series_30d[health!.fee_series_30d.length - 1]?.day}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function bufferStatusLabel(status: string): string {
  if (status === 'ok') return 'ok';
  if (status === 'low') return 'baixo';
  if (status === 'critical') return 'crítico';
  if (status === 'rpc') return 'RPC';
  return status;
}

function networkKindLabel(kind: string): string {
  if (kind === 'WITHDRAWAL') return 'saque';
  if (kind === 'SWEEP') return 'varredura';
  if (kind === 'DEX_DEPOSIT') return 'depósito DEX';
  if (kind === 'GAS_TOPUP') return 'recarga de gás';
  return kind;
}

function isCoinFmt(amount: string, coin: string): string {
  return COINS.includes(coin as Coin) ? formatAmount(amount, coin as Coin) : amount;
}

import { useMemo, useState, type ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { api } from '../lib/api.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COIN_CONFIG, COINS, formatAmount, isCoin, safeBigInt, type Coin } from '@/shared';
import { AdminTreasuryMonitor } from '../components/AdminTreasuryMonitor.js';

interface CoinVolumeItem {
  coin: string;
  count: number;
  total_amount: string;
  total_fee?: string;
}

interface RecentDepositItem {
  id: string;
  email: string;
  coin: string;
  amount: string;
  tx_hash: string | null;
  status: string;
  confirmations: number;
  created_at: string;
}

interface RecentWithdrawalItem {
  id: string;
  user_id: string;
  email: string;
  coin: string;
  to_address: string;
  amount: string;
  fee: string;
  status: string;
  tx_hash: string | null;
  requires_approval: boolean;
  created_at: string;
}

interface ServerResourceStats {
  uptime_seconds: number;
  cpu_load_1m: number;
  cpu_load_5m: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_pct: number;
  db_connections_active: number;
  db_connections_idle: number;
  db_connections_max: number;
  status: string;
}

interface DashboardStats {
  total_users: number;
  new_users_24h: number;
  active_users_24h: number;
  total_wallets: number;
  total_merchants: number;
  total_faucet_sites: number;
  pending_withdrawals: number;
  total_deposits_count: number;
  total_withdrawals_count: number;
  deposits_by_coin: CoinVolumeItem[];
  withdrawals_by_coin: CoinVolumeItem[];
  user_balances: { coin: string; balance: string }[];
  house_balances: { coin: string; balance: string }[];
  recent_deposits: RecentDepositItem[];
  recent_withdrawals: RecentWithdrawalItem[];
  server: ServerResourceStats;
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
  faucet_unique_users: number;
  faucet_by_coin: EconCoinFlow[];
  gateway_created: number;
  gateway_paid: number;
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

type MainTab = 'economia' | 'operacao';
type ActivityTab = 'deposits' | 'withdrawals';
type EconPeriod = '24h' | 'all';

const STATS_REFETCH_MS = 10_000;

function formatUptime(seconds: number): string {
  if (!seconds || seconds <= 0) return '…';
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m`;
}

function explorerTx(coin: string, hash: string): string | null {
  const c = coin.toUpperCase();
  if (c === 'POL' || c === 'USDT' || c === 'USDC') return `https://polygonscan.com/tx/${hash}`;
  if (c === 'BTC') return `https://mempool.space/tx/${hash}`;
  if (c === 'LTC') return `https://litecoinspace.org/tx/${hash}`;
  if (c === 'DOGE') return `https://dogechain.info/tx/${hash}`;
  if (c === 'BCH') return `https://blockchair.com/bitcoin-cash/transaction/${hash}`;
  if (c === 'SOL') return `https://solscan.io/tx/${hash}`;
  if (c === 'DGB') return `https://digiexplorer.info/tx/${hash}`;
  if (c === 'ZER') return `https://zerochain.info/tx/${hash}`;
  return null;
}

function shortHash(h: string): string {
  return h.length > 14 ? `${h.slice(0, 6)}…${h.slice(-6)}` : h;
}

function sumFees(rows: EconCoinFlow[]): bigint {
  return rows.reduce((acc, r) => acc + safeBigInt(r.fees), 0n);
}

function sumVolume(rows: EconCoinFlow[]): bigint {
  return rows.reduce((acc, r) => acc + safeBigInt(r.volume), 0n);
}

function fmtCoin(amount: string | number | bigint, coin: string): string {
  if (isCoin(coin)) return formatAmount(amount.toString(), coin as Coin);
  return amount.toString();
}

function CoinFlowTable({
  rows,
  empty,
  showFees = true,
  volumeLabel = 'Volume',
  feesLabel = 'Taxas',
}: {
  rows: EconCoinFlow[];
  empty: string;
  showFees?: boolean;
  volumeLabel?: string;
  feesLabel?: string;
}) {
  if (!rows.length) {
    return <p className="py-8 text-center text-xs text-ink-muted">{empty}</p>;
  }
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left text-xs">
        <thead className="text-[10px] uppercase tracking-wider text-ink-muted font-bold bg-surface/50">
          <tr>
            <th className="py-2 px-4">Moeda</th>
            <th className="py-2 px-3 text-right">Ops</th>
            <th className="py-2 px-3 text-right">{volumeLabel}</th>
            {showFees ? <th className="py-2 px-4 text-right">{feesLabel}</th> : null}
          </tr>
        </thead>
        <tbody className="divide-y divide-border font-mono">
          {rows.map((r) => {
            const coin = isCoin(r.coin) ? (r.coin as Coin) : null;
            return (
              <tr key={r.coin} className="hover:bg-surface/40">
                <td className="py-2.5 px-4 font-sans">
                  <div className="flex items-center gap-2">
                    {coin ? <img src={coinLogo(coin)} alt="" className="h-5 w-5 rounded-full" /> : null}
                    <span className="font-black text-ink">{r.coin}</span>
                  </div>
                </td>
                <td className="py-2.5 px-3 text-right tabular-nums">{r.count}</td>
                <td className="py-2.5 px-3 text-right font-bold tabular-nums">
                  {coin ? formatAmount(r.volume, coin) : r.volume}
                </td>
                {showFees ? (
                  <td className="py-2.5 px-4 text-right font-bold tabular-nums text-emerald-700">
                    {coin ? formatAmount(r.fees, coin) : r.fees}
                  </td>
                ) : null}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function StreamCard({
  title,
  subtitle,
  icon,
  accent,
  children,
  footer,
}: {
  title: string;
  subtitle: string;
  icon: string;
  accent: string;
  children: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <section className="rounded-2xl border border-border bg-paper shadow-xs overflow-hidden flex flex-col">
      <div className="flex items-start gap-3 px-4 py-3 border-b border-border bg-gradient-to-r from-surface/80 to-transparent">
        <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl text-base ${accent}`}>
          <i className={`bi ${icon}`} />
        </div>
        <div className="min-w-0 flex-1">
          <h3 className="text-sm font-black text-ink leading-tight">{title}</h3>
          <p className="text-[11px] text-ink-muted mt-0.5">{subtitle}</p>
        </div>
        {footer}
      </div>
      <div className="flex-1">{children}</div>
    </section>
  );
}

function Kpi({
  label,
  value,
  hint,
  tone = 'default',
  icon,
  footer,
}: {
  label: string;
  value: string | number;
  hint?: string;
  tone?: 'default' | 'good' | 'warn' | 'info' | 'bad';
  icon: string;
  footer?: ReactNode;
}) {
  const valueCls =
    tone === 'good'
      ? 'text-emerald-600'
      : tone === 'warn'
        ? 'text-amber-600'
        : tone === 'info'
          ? 'text-sky-600'
          : tone === 'bad'
            ? 'text-rose-600'
            : 'text-ink';
  const iconCls =
    tone === 'good'
      ? 'bg-emerald-500/10 text-emerald-600'
      : tone === 'warn'
        ? 'bg-amber-500/10 text-amber-600'
        : tone === 'info'
          ? 'bg-sky-500/10 text-sky-600'
          : tone === 'bad'
            ? 'bg-rose-500/10 text-rose-600'
            : 'bg-bitcoin/10 text-bitcoin';

  return (
    <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
      <div className="flex items-start justify-between gap-2">
        <div>
          <p className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">{label}</p>
          <p className={`mt-1 text-2xl font-black font-mono tabular-nums ${valueCls}`}>{value}</p>
          {hint && <p className="mt-0.5 text-[11px] text-ink-muted">{hint}</p>}
        </div>
        <div className={`h-9 w-9 rounded-xl flex items-center justify-center text-sm shrink-0 ${iconCls}`}>
          <i className={`bi ${icon}`} />
        </div>
      </div>
      {footer && <div className="mt-3 pt-3 border-t border-border/70 text-[11px]">{footer}</div>}
    </div>
  );
}

function RevenueHero({
  title,
  hint,
  rows,
  tone,
  icon,
  emptyHint,
}: {
  title: string;
  hint: string;
  rows: EconCoinFlow[];
  tone: 'good' | 'bad' | 'info';
  icon: string;
  emptyHint: string;
}) {
  const toneCls =
    tone === 'good'
      ? 'from-emerald-500/15 via-paper to-paper border-emerald-500/25'
      : tone === 'bad'
        ? 'from-rose-500/12 via-paper to-paper border-rose-500/25'
        : 'from-sky-500/12 via-paper to-paper border-sky-500/25';
  const iconCls =
    tone === 'good'
      ? 'bg-emerald-500/15 text-emerald-700'
      : tone === 'bad'
        ? 'bg-rose-500/15 text-rose-700'
        : 'bg-sky-500/15 text-sky-700';

  return (
    <div className={`rounded-2xl border bg-gradient-to-br p-4 shadow-xs ${toneCls}`}>
      <div className="flex items-center gap-2 mb-3">
        <div className={`flex h-9 w-9 items-center justify-center rounded-xl ${iconCls}`}>
          <i className={`bi ${icon}`} />
        </div>
        <div>
          <div className="text-sm font-black text-ink">{title}</div>
          <div className="text-[11px] text-ink-muted">{hint}</div>
        </div>
      </div>
      {rows.length === 0 ? (
        <p className="text-xs text-ink-muted py-2">{emptyHint}</p>
      ) : (
        <ul className="space-y-2">
          {rows.map((r) => {
            const coin = isCoin(r.coin) ? (r.coin as Coin) : null;
            const amt = tone === 'bad' ? r.volume : r.fees;
            return (
              <li key={r.coin} className="flex items-center justify-between gap-2 text-xs">
                <span className="inline-flex items-center gap-1.5 font-bold text-ink">
                  {coin ? <img src={coinLogo(coin)} alt="" className="h-4 w-4 rounded-full" /> : null}
                  {r.coin}
                  <span className="text-ink-muted font-mono font-normal">{r.count}×</span>
                </span>
                <span
                  className={`font-mono font-black tabular-nums ${
                    tone === 'bad' ? 'text-rose-700' : 'text-emerald-700'
                  }`}
                >
                  {tone === 'bad' ? '−' : '+'}
                  {coin ? formatAmount(amt, coin) : amt}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

export function AdminOverviewPage() {
  const queryClient = useQueryClient();
  const [mainTab, setMainTab] = useState<MainTab>('economia');
  const [tab, setTab] = useState<ActivityTab>('deposits');
  const [econPeriod, setEconPeriod] = useState<EconPeriod>('all');
  const [copied, setCopied] = useState<string | null>(null);
  const [showAllCoins, setShowAllCoins] = useState(false);

  const { data: stats, isError, refetch, isFetching } = useQuery<DashboardStats>({
    queryKey: ['admin-stats'],
    queryFn: () => api<DashboardStats>('/admin/stats'),
    refetchInterval: STATS_REFETCH_MS,
  });

  const econQ = useQuery<PlatformEconomics>({
    queryKey: ['admin-economics'],
    queryFn: () => api<PlatformEconomics>('/admin/economics'),
    refetchInterval: STATS_REFETCH_MS,
  });

  const win = econPeriod === '24h' ? econQ.data?.last_24h : econQ.data?.all_time;
  const winAlt = econPeriod === '24h' ? econQ.data?.all_time : econQ.data?.last_24h;

  const depositVol = useMemo(() => {
    const m: Record<string, CoinVolumeItem> = {};
    for (const d of stats?.deposits_by_coin ?? []) m[d.coin.toUpperCase()] = d;
    return m;
  }, [stats?.deposits_by_coin]);

  const withdrawalVol = useMemo(() => {
    const m: Record<string, CoinVolumeItem> = {};
    for (const w of stats?.withdrawals_by_coin ?? []) m[w.coin.toUpperCase()] = w;
    return m;
  }, [stats?.withdrawals_by_coin]);

  const userBal = useMemo(() => {
    const m: Record<string, string> = {};
    for (const b of stats?.user_balances ?? []) m[b.coin.toUpperCase()] = b.balance;
    return m;
  }, [stats?.user_balances]);

  const coinRows = useMemo(() => {
    return COINS.filter((c) => {
      if (showAllCoins) return true;
      const dep = depositVol[c];
      const wdr = withdrawalVol[c];
      const bal = userBal[c] ?? '0';
      return (
        safeBigInt(dep?.total_amount) > 0n ||
        (dep?.count ?? 0) > 0 ||
        safeBigInt(wdr?.total_amount) > 0n ||
        (wdr?.count ?? 0) > 0 ||
        safeBigInt(bal) > 0n
      );
    });
  }, [depositVol, withdrawalVol, userBal, showAllCoins]);

  const pending = stats?.pending_withdrawals ?? 0;
  const memPct = stats?.server.memory_pct ?? 0;

  const copy = (text: string) => {
    void navigator.clipboard.writeText(text);
    setCopied(text);
    setTimeout(() => setCopied(null), 1600);
  };

  const gatewayFeeRows = win?.gateway_by_coin.filter((r) => safeBigInt(r.fees) > 0n) ?? [];
  const faucetCostRows = win?.faucet_by_coin.filter((r) => safeBigInt(r.volume) > 0n) ?? [];
  const wdFeeRows = win?.withdrawals_by_coin.filter((r) => safeBigInt(r.fees) > 0n) ?? [];
  const swapFeeRows = win?.swap_fees_by_coin.filter((r) => safeBigInt(r.fees) > 0n) ?? [];

  return (
    <div className="space-y-6 pb-16 max-w-6xl mx-auto">
      <header className="flex flex-col sm:flex-row sm:items-end sm:justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-black tracking-tight text-ink">Visão geral</h1>
            <span className="inline-flex items-center gap-1 rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2 py-0.5 text-[10px] font-black uppercase tracking-wide text-emerald-700">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
              Rede principal
            </span>
          </div>
          <p className="mt-1 text-xs text-ink-muted">
            Receita, custos e volume da plataforma — faucet, gateway, depósitos e saques.
          </p>
        </div>
        <button
          type="button"
          onClick={() => {
            void refetch();
            void econQ.refetch();
            void queryClient.invalidateQueries({ queryKey: ['admin-treasury-wallets'] });
          }}
          disabled={isFetching || econQ.isFetching}
          className="inline-flex items-center gap-2 self-start rounded-xl border border-border bg-paper px-3 py-2 text-xs font-bold text-ink hover:bg-surface disabled:opacity-50"
        >
          <i className={`bi bi-arrow-repeat ${isFetching || econQ.isFetching ? 'animate-spin text-bitcoin' : ''}`} />
          Atualizar
        </button>
      </header>

      {(isError || econQ.isError) && (
        <div className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-xs font-bold text-rose-700">
          Falha ao carregar estatísticas.
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2 border-b border-border pb-1">
        {(
          [
            ['economia', 'Economia'],
            ['operacao', 'Operação'],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            onClick={() => setMainTab(id)}
            className={`rounded-t-xl px-4 py-2 text-xs font-black transition ${
              mainTab === id
                ? 'bg-paper text-ink border border-border border-b-paper -mb-px shadow-xs'
                : 'text-ink-muted hover:text-ink'
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {mainTab === 'economia' && (
        <div className="space-y-5">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 className="text-sm font-black text-ink">Fluxos financeiros</h2>
              <p className="text-[11px] text-ink-muted">
                Valores em unidades nativas do ledger (sem USD). Faucet = custo HOUSE · Gateway = taxa 0,25%.
              </p>
            </div>
            <div className="inline-flex rounded-xl border border-border bg-surface p-0.5">
              {(
                [
                  ['24h', '24 horas'],
                  ['all', 'Histórico'],
                ] as const
              ).map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => setEconPeriod(id)}
                  className={`rounded-lg px-3 py-1.5 text-xs font-bold transition ${
                    econPeriod === id ? 'bg-paper text-ink shadow-xs' : 'text-ink-muted hover:text-ink'
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          {econQ.isLoading && !win ? (
            <div className="py-16 text-center text-xs text-ink-muted animate-pulse">Carregando economia…</div>
          ) : (
            <>
              <div className="grid sm:grid-cols-2 xl:grid-cols-4 gap-3">
                <Kpi
                  label="Gateway — invoices pagas"
                  value={win?.gateway_paid ?? 0}
                  hint={`${win?.gateway_created ?? 0} criadas · taxa retida na confirmação`}
                  tone="good"
                  icon="bi-shop"
                  footer={
                    <Link to="/admin/merchants" className="font-bold text-bitcoin hover:underline">
                      Abrir comerciantes →
                    </Link>
                  }
                />
                <Kpi
                  label="Faucet — resgates"
                  value={win?.faucet_claims ?? 0}
                  hint={`${win?.faucet_unique_users ?? 0} usuários únicos · debitado da HOUSE`}
                  tone="bad"
                  icon="bi-droplet-fill"
                />
                <Kpi
                  label="Depósitos custódia"
                  value={win?.deposits_count ?? 0}
                  hint="créditos on-chain em wallets pessoais"
                  tone="info"
                  icon="bi-arrow-down-left-circle-fill"
                />
                <Kpi
                  label="Saques + swaps"
                  value={`${win?.withdrawals_count ?? 0} / ${win?.swap_count ?? 0}`}
                  hint="saques confirmados · swaps completed"
                  icon="bi-arrow-left-right"
                />
              </div>

              <div className="grid md:grid-cols-2 xl:grid-cols-4 gap-3">
                <RevenueHero
                  title="Receita gateway"
                  hint="fee_amount das invoices CONFIRMED"
                  rows={gatewayFeeRows}
                  tone="good"
                  icon="bi-cash-stack"
                  emptyHint="Nenhuma taxa de gateway ainda."
                />
                <RevenueHero
                  title="Custo faucet"
                  hint="Valor pago aos usuários (HOUSE)"
                  rows={faucetCostRows}
                  tone="bad"
                  icon="bi-droplet-half"
                  emptyHint="Nenhum claim de faucet no período."
                />
                <RevenueHero
                  title="Taxas de saque"
                  hint="fee_amount em saques broadcast/confirmados"
                  rows={wdFeeRows}
                  tone="good"
                  icon="bi-send-check"
                  emptyHint="Sem taxas de saque no período."
                />
                <RevenueHero
                  title="Taxas de swap"
                  hint="platform_fee_amount em swaps COMPLETED"
                  rows={swapFeeRows}
                  tone="info"
                  icon="bi-currency-exchange"
                  emptyHint="Sem taxas de swap no período."
                />
              </div>

              <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin">
                      <i className="bi bi-graph-up-arrow" />
                    </div>
                    <div>
                      <h3 className="text-sm font-black text-ink">Margem de taxas</h3>
                      <p className="text-[11px] text-ink-muted">
                        Taxas cobradas − custo de rede (miner/gas). Faucet fica fora da margem de taxas.
                      </p>
                    </div>
                  </div>
                  {(() => {
                    const margins = win?.fee_margin_by_coin ?? [];
                    const unhealthy = margins.filter((m) => !m.healthy).length;
                    if (margins.length === 0) return null;
                    return unhealthy > 0 ? (
                      <span className="rounded-full border border-rose-500/30 bg-rose-500/10 px-2.5 py-0.5 text-[10px] font-black uppercase tracking-wide text-rose-700">
                        {unhealthy} coin{unhealthy > 1 ? 's' : ''} no vermelho
                      </span>
                    ) : (
                      <span className="rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-0.5 text-[10px] font-black uppercase tracking-wide text-emerald-700">
                        Saudável
                      </span>
                    );
                  })()}
                </div>
                <p className="text-[10px] text-ink-muted">
                  Custo de rede só conta eventos gravados após o deploy (saques, sweeps, DEX). Histórico antigo não tem fee on-chain.
                </p>
                {(win?.fee_margin_by_coin ?? []).length === 0 ? (
                  <p className="py-6 text-center text-xs text-ink-muted">
                    Sem movimento de taxas/rede no período.
                  </p>
                ) : (
                  <div className="overflow-x-auto">
                    <table className="w-full text-left text-xs">
                      <thead className="text-[10px] uppercase tracking-wider text-ink-muted font-bold bg-surface/50">
                        <tr>
                          <th className="py-2 px-3">Moeda</th>
                          <th className="py-2 px-3 text-right">Taxas +</th>
                          <th className="py-2 px-3 text-right">Rede −</th>
                          <th className="py-2 px-3 text-right">Margem</th>
                          <th className="py-2 px-3 text-right">Op. (−faucet)</th>
                          <th className="py-2 px-3 text-right">Status</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-border font-mono">
                        {(win?.fee_margin_by_coin ?? []).map((m) => {
                          const coin = isCoin(m.coin) ? (m.coin as Coin) : null;
                          const marginNeg = safeBigInt(m.fee_margin) < 0n;
                          return (
                            <tr key={m.coin} className="hover:bg-surface/40">
                              <td className="py-2.5 px-3 font-sans">
                                <div className="flex items-center gap-2">
                                  {coin ? <img src={coinLogo(coin)} alt="" className="h-5 w-5 rounded-full" /> : null}
                                  <span className="font-black text-ink">{m.coin}</span>
                                </div>
                              </td>
                              <td className="py-2.5 px-3 text-right tabular-nums text-emerald-700 font-bold">
                                {fmtCoin(m.fees_earned, m.coin)}
                              </td>
                              <td className="py-2.5 px-3 text-right tabular-nums text-rose-700 font-bold">
                                {fmtCoin(m.network_paid, m.coin)}
                              </td>
                              <td
                                className={`py-2.5 px-3 text-right tabular-nums font-bold ${
                                  marginNeg ? 'text-rose-700' : 'text-emerald-700'
                                }`}
                              >
                                {fmtCoin(m.fee_margin, m.coin)}
                              </td>
                              <td className="py-2.5 px-3 text-right tabular-nums text-ink-muted">
                                {fmtCoin(m.operating_margin, m.coin)}
                              </td>
                              <td className="py-2.5 px-3 text-right font-sans">
                                {m.healthy ? (
                                  <span className="text-[10px] font-black uppercase text-emerald-700">Ok</span>
                                ) : (
                                  <span className="text-[10px] font-black uppercase text-rose-700">Prejuízo</span>
                                )}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                )}
                {(win?.network_by_kind ?? []).length > 0 && (
                  <div className="pt-2 border-t border-border">
                    <div className="text-[10px] font-bold uppercase text-ink-muted mb-1.5">Rede por tipo</div>
                    <ul className="grid sm:grid-cols-2 gap-1 text-xs font-mono">
                      {(win?.network_by_kind ?? []).map((n) => (
                        <li key={`${n.coin}-${n.kind}`} className="flex justify-between gap-2 rounded-lg bg-surface/60 px-2 py-1">
                          <span className="text-ink-muted">
                            {n.coin} ·{' '}
                            {n.kind === 'WITHDRAWAL'
                              ? 'saque'
                              : n.kind === 'SWEEP'
                                ? 'varredura'
                                : n.kind === 'DEX_DEPOSIT'
                                  ? 'depósito DEX'
                                  : n.kind === 'GAS_TOPUP'
                                    ? 'recarga de gás'
                                    : n.kind}{' '}
                            ×{n.count}
                          </span>
                          <span className="font-bold text-rose-700">{fmtCoin(n.amount, n.coin)}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>

              {winAlt && econPeriod === '24h' && (
                <p className="text-[11px] text-ink-muted">
                  Histórico: gateway {winAlt.gateway_paid} pagas · faucet {winAlt.faucet_claims} resgates · depósitos{' '}
                  {winAlt.deposits_count} · saques {winAlt.withdrawals_count}.
                </p>
              )}

              <div className="grid lg:grid-cols-2 gap-4">
                <StreamCard
                  title="Gateway de depósito"
                  subtitle="Volume pago pelos clientes + taxa da plataforma"
                  icon="bi-shop-window"
                  accent="bg-emerald-500/10 text-emerald-700"
                  footer={
                    <div className="text-right text-[10px] font-mono text-ink-muted">
                      <div>taxas Σ {gatewayFeeRows.length ? 'por moeda ↓' : '0'}</div>
                    </div>
                  }
                >
                  <CoinFlowTable
                    rows={win?.gateway_by_coin ?? []}
                    empty="Sem invoices confirmadas. Quando o gateway processar pagamentos, volume e taxas aparecem aqui."
                    volumeLabel="Volume pago"
                    feesLabel="Receita (taxa)"
                  />
                </StreamCard>

                <StreamCard
                  title="Faucet"
                  subtitle="Saída de inventário HOUSE para usuários"
                  icon="bi-droplet"
                  accent="bg-rose-500/10 text-rose-700"
                >
                  <CoinFlowTable
                    rows={win?.faucet_by_coin ?? []}
                    empty="Sem resgates. O faucet debita a HOUSE — isto é custo operacional, não receita."
                    showFees={false}
                    volumeLabel="Distribuído"
                  />
                </StreamCard>

                <StreamCard
                  title="Depósitos (carteira)"
                  subtitle="Entrada on-chain creditada em PERSONAL"
                  icon="bi-safe"
                  accent="bg-sky-500/10 text-sky-700"
                >
                  <CoinFlowTable
                    rows={win?.deposits_by_coin ?? []}
                    empty="Nenhum depósito creditado (amount > 0) no período."
                    showFees={false}
                    volumeLabel="Creditado"
                  />
                </StreamCard>

                <StreamCard
                  title="Saques"
                  subtitle="Volume enviado on-chain + taxa cobrada"
                  icon="bi-box-arrow-up-right"
                  accent="bg-bitcoin/10 text-bitcoin"
                >
                  <CoinFlowTable
                    rows={win?.withdrawals_by_coin ?? []}
                    empty="Nenhum saque broadcast/confirmado no período."
                    volumeLabel="Enviado"
                    feesLabel="Taxa"
                  />
                </StreamCard>
              </div>

              <StreamCard
                title="Swaps DEX"
                subtitle="Volume from_amount e platform fee (COMPLETED)"
                icon="bi-arrow-left-right"
                accent="bg-violet-500/10 text-violet-700"
              >
                <CoinFlowTable
                  rows={win?.swap_fees_by_coin ?? []}
                  empty="Nenhum swap completed no período."
                  volumeLabel="From volume"
                  feesLabel="Platform fee"
                />
              </StreamCard>
            </>
          )}
        </div>
      )}

      {mainTab === 'operacao' && (
        <div className="space-y-6">
          <section className="grid grid-cols-2 lg:grid-cols-4 gap-3">
            <Kpi
              label="Usuários"
              value={stats?.total_users ?? 0}
              hint={`+${stats?.new_users_24h ?? 0} hoje · ${stats?.active_users_24h ?? 0} ativos`}
              icon="bi-people-fill"
            />
            <Kpi
              label="Depósitos"
              value={stats?.total_deposits_count ?? 0}
              hint="créditos on-chain"
              tone="good"
              icon="bi-arrow-down-left-circle-fill"
            />
            <Kpi
              label="Saques"
              value={stats?.total_withdrawals_count ?? 0}
              hint="broadcast / confirmados"
              tone="info"
              icon="bi-arrow-up-right-circle-fill"
            />
            <Kpi
              label="Pendentes"
              value={pending}
              hint={pending > 0 ? 'precisam revisão' : 'fila limpa'}
              tone={pending > 0 ? 'warn' : 'good'}
              icon="bi-shield-exclamation"
              footer={
                pending > 0 ? (
                  <Link to="/admin/withdrawals" className="font-bold text-amber-700 hover:underline">
                    Revisar saques →
                  </Link>
                ) : (
                  <span className="font-bold text-emerald-700">Tudo em dia</span>
                )
              }
            />
          </section>

          <section className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
            <div className="flex flex-wrap items-center justify-between gap-3 mb-3">
              <h2 className="text-sm font-black text-ink">Infraestrutura</h2>
              <div className="flex flex-wrap items-center gap-2 text-[11px]">
                <span className="rounded-lg border border-border bg-surface px-2 py-1 font-mono text-ink">
                  up {formatUptime(stats?.server.uptime_seconds ?? 0)}
                </span>
                <span className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-2 py-1 font-bold text-emerald-700">
                  {stats?.server.status ?? 'HEALTHY'}
                </span>
              </div>
            </div>

            <div className="grid sm:grid-cols-3 gap-3">
              <div className="rounded-xl border border-border/80 bg-surface/40 p-3">
                <div className="flex justify-between text-[11px] font-bold text-ink mb-1.5">
                  <span>RAM</span>
                  <span className="font-mono">{memPct}%</span>
                </div>
                <div className="h-1.5 rounded-full bg-border overflow-hidden">
                  <div
                    className={`h-full rounded-full ${
                      memPct > 85 ? 'bg-rose-500' : memPct > 65 ? 'bg-amber-500' : 'bg-bitcoin'
                    }`}
                    style={{ width: `${Math.min(memPct, 100)}%` }}
                  />
                </div>
                <p className="mt-1.5 text-[10px] text-ink-muted font-mono">
                  {stats?.server.memory_used_mb ?? 0} / {stats?.server.memory_total_mb ?? 0} MB
                </p>
              </div>

              <div className="rounded-xl border border-border/80 bg-surface/40 p-3">
                <p className="text-[11px] font-bold text-ink mb-1.5">Carga</p>
                <div className="flex gap-2 font-mono">
                  <div className="flex-1 rounded-lg border border-border bg-paper px-2 py-1.5 text-center">
                    <div className="text-[9px] uppercase text-ink-muted font-bold">1m</div>
                    <div className="text-sm font-black text-ink">
                      {(stats?.server.cpu_load_1m ?? 0).toFixed(2)}
                    </div>
                  </div>
                  <div className="flex-1 rounded-lg border border-border bg-paper px-2 py-1.5 text-center">
                    <div className="text-[9px] uppercase text-ink-muted font-bold">5m</div>
                    <div className="text-sm font-black text-ink">
                      {(stats?.server.cpu_load_5m ?? 0).toFixed(2)}
                    </div>
                  </div>
                </div>
              </div>

              <div className="rounded-xl border border-border/80 bg-surface/40 p-3">
                <div className="flex justify-between text-[11px] font-bold text-ink mb-1.5">
                  <span>Postgres</span>
                  <span className="font-mono">
                    {stats?.server.db_connections_active ?? 0}/{stats?.server.db_connections_max ?? 50}
                  </span>
                </div>
                <div className="h-1.5 rounded-full bg-border overflow-hidden">
                  <div
                    className="h-full rounded-full bg-emerald-500"
                    style={{
                      width: `${Math.min(
                        ((stats?.server.db_connections_active ?? 0) /
                          (stats?.server.db_connections_max ?? 50)) *
                          100,
                        100,
                      )}%`,
                    }}
                  />
                </div>
                <p className="mt-1.5 text-[10px] text-ink-muted font-mono">
                  idle {stats?.server.db_connections_idle ?? 0}
                </p>
              </div>
            </div>
          </section>

          <div className="grid lg:grid-cols-5 gap-4">
            <section className="lg:col-span-3 rounded-2xl border border-border bg-paper shadow-xs overflow-hidden">
              <div className="flex items-center justify-between gap-2 px-4 py-3 border-b border-border">
                <div>
                  <h2 className="text-sm font-black text-ink">Fluxo por moeda</h2>
                  <p className="text-[11px] text-ink-muted">Depósito · saque · custódia</p>
                </div>
                <label className="inline-flex items-center gap-1.5 text-[11px] font-bold text-ink-muted cursor-pointer">
                  <input
                    type="checkbox"
                    checked={showAllCoins}
                    onChange={(e) => setShowAllCoins(e.target.checked)}
                    className="rounded border-border"
                  />
                  Todas
                </label>
              </div>

              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs">
                  <thead className="text-[10px] uppercase tracking-wider text-ink-muted font-bold bg-surface/50">
                    <tr>
                      <th className="py-2 px-4">Ativo</th>
                      <th className="py-2 px-3 text-right">Entrada</th>
                      <th className="py-2 px-3 text-right">Saída</th>
                      <th className="py-2 px-4 text-right">Custódia</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border font-mono">
                    {coinRows.length === 0 ? (
                      <tr>
                        <td colSpan={4} className="py-8 text-center text-ink-muted font-sans">
                          Sem movimento ainda.
                        </td>
                      </tr>
                    ) : (
                      coinRows.map((c) => {
                        const dep = depositVol[c];
                        const wdr = withdrawalVol[c];
                        const bal = userBal[c] ?? '0';
                        return (
                          <tr key={c} className="hover:bg-surface/40">
                            <td className="py-2.5 px-4 font-sans">
                              <div className="flex items-center gap-2">
                                <img src={coinLogo(c)} alt="" className="h-5 w-5 rounded-full" />
                                <div>
                                  <div className="font-black text-ink leading-none">{c}</div>
                                  <div className="text-[10px] text-ink-muted font-bold">
                                    {COIN_CONFIG[c].name}
                                  </div>
                                </div>
                              </div>
                            </td>
                            <td className="py-2.5 px-3 text-right">
                              <div className="font-bold text-emerald-600">
                                {dep ? formatAmount(dep.total_amount, c) : '0'}
                              </div>
                              <div className="text-[10px] text-ink-muted font-sans">
                                {dep?.count ? `${dep.count}×` : '—'}
                              </div>
                            </td>
                            <td className="py-2.5 px-3 text-right">
                              <div className="font-bold text-sky-600">
                                {wdr ? formatAmount(wdr.total_amount, c) : '0'}
                              </div>
                              <div className="text-[10px] text-ink-muted font-sans">
                                {wdr?.count ? `${wdr.count}×` : '—'}
                              </div>
                            </td>
                            <td className="py-2.5 px-4 text-right font-bold text-ink">
                              {formatAmount(bal, c)}
                            </td>
                          </tr>
                        );
                      })
                    )}
                  </tbody>
                </table>
              </div>
            </section>

            <div className="lg:col-span-2">
              <AdminTreasuryMonitor />
            </div>
          </div>

          <section className="rounded-2xl border border-border bg-paper shadow-xs overflow-hidden">
            <div className="flex flex-wrap items-center justify-between gap-2 px-4 py-3 border-b border-border">
              <div className="inline-flex rounded-xl border border-border bg-surface p-0.5">
                <button
                  type="button"
                  onClick={() => setTab('deposits')}
                  className={`rounded-lg px-3 py-1.5 text-xs font-bold transition ${
                    tab === 'deposits' ? 'bg-paper text-ink shadow-xs' : 'text-ink-muted hover:text-ink'
                  }`}
                >
                  Depósitos ({stats?.recent_deposits?.length ?? 0})
                </button>
                <button
                  type="button"
                  onClick={() => setTab('withdrawals')}
                  className={`rounded-lg px-3 py-1.5 text-xs font-bold transition ${
                    tab === 'withdrawals' ? 'bg-paper text-ink shadow-xs' : 'text-ink-muted hover:text-ink'
                  }`}
                >
                  Saques ({stats?.recent_withdrawals?.length ?? 0})
                </button>
              </div>
              <span className="text-[10px] font-mono text-ink-muted">auto · 10s</span>
            </div>

            <div className="overflow-x-auto">
              {tab === 'deposits' &&
                (!(stats?.recent_deposits?.length) ? (
                  <p className="py-10 text-center text-xs text-ink-muted">Nenhum depósito recente.</p>
                ) : (
                  <table className="w-full text-left text-xs">
                    <thead className="text-[10px] uppercase tracking-wider text-ink-muted font-bold bg-surface/40">
                      <tr>
                        <th className="py-2 px-4">Quando</th>
                        <th className="py-2 px-3">Usuário</th>
                        <th className="py-2 px-3">Moeda</th>
                        <th className="py-2 px-3 text-right">Valor</th>
                        <th className="py-2 px-4 text-right">Tx</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border">
                      {stats!.recent_deposits.map((dep) => {
                        const c = dep.coin.toUpperCase() as Coin;
                        const tx = dep.tx_hash ?? '';
                        const url = tx ? explorerTx(dep.coin, tx) : null;
                        return (
                          <tr key={dep.id} className="hover:bg-surface/40">
                            <td className="py-2.5 px-4 font-mono text-[11px] text-ink-muted whitespace-nowrap">
                              {new Date(dep.created_at).toLocaleString('pt-BR', {
                                day: '2-digit',
                                month: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit',
                              })}
                            </td>
                            <td className="py-2.5 px-3 font-bold text-ink truncate max-w-[140px]" title={dep.email}>
                              {dep.email}
                            </td>
                            <td className="py-2.5 px-3">
                              <span className="inline-flex items-center gap-1.5 font-black">
                                <img src={coinLogo(c)} alt="" className="h-4 w-4 rounded-full" />
                                {dep.coin}
                              </span>
                            </td>
                            <td className="py-2.5 px-3 text-right font-mono font-bold text-emerald-600">
                              +{formatAmount(dep.amount, c)}
                            </td>
                            <td className="py-2.5 px-4 text-right">
                              {tx ? (
                                <span className="inline-flex items-center justify-end gap-1">
                                  <button
                                    type="button"
                                    onClick={() => copy(tx)}
                                    className="rounded-lg border border-border px-2 py-0.5 font-mono text-[11px] hover:bg-surface"
                                  >
                                    {shortHash(tx)}
                                    <i
                                      className={`bi ml-1 ${copied === tx ? 'bi-check text-emerald-600' : 'bi-copy text-ink-muted'}`}
                                    />
                                  </button>
                                  {url && (
                                    <a href={url} target="_blank" rel="noreferrer" className="text-bitcoin p-1">
                                      <i className="bi bi-box-arrow-up-right text-[10px]" />
                                    </a>
                                  )}
                                </span>
                              ) : (
                                <span className="text-ink-muted">—</span>
                              )}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                ))}

              {tab === 'withdrawals' &&
                (!(stats?.recent_withdrawals?.length) ? (
                  <p className="py-10 text-center text-xs text-ink-muted">Nenhum saque recente.</p>
                ) : (
                  <table className="w-full text-left text-xs">
                    <thead className="text-[10px] uppercase tracking-wider text-ink-muted font-bold bg-surface/40">
                      <tr>
                        <th className="py-2 px-4">Quando</th>
                        <th className="py-2 px-3">Usuário</th>
                        <th className="py-2 px-3">Moeda</th>
                        <th className="py-2 px-3 text-right">Valor</th>
                        <th className="py-2 px-3 text-center">Status</th>
                        <th className="py-2 px-4 text-right">Tx</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border">
                      {stats!.recent_withdrawals.map((w) => {
                        const c = w.coin.toUpperCase() as Coin;
                        const url = w.tx_hash ? explorerTx(w.coin, w.tx_hash) : null;
                        const ok = w.status === 'CONFIRMED' || w.status === 'BROADCASTED';
                        const fail = w.status === 'FAILED';
                        const pend = w.status === 'PENDING';
                        return (
                          <tr key={w.id} className="hover:bg-surface/40">
                            <td className="py-2.5 px-4 font-mono text-[11px] text-ink-muted whitespace-nowrap">
                              {new Date(w.created_at).toLocaleString('pt-BR', {
                                day: '2-digit',
                                month: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit',
                              })}
                            </td>
                            <td className="py-2.5 px-3 font-bold text-ink truncate max-w-[140px]" title={w.email}>
                              {w.email}
                            </td>
                            <td className="py-2.5 px-3">
                              <span className="inline-flex items-center gap-1.5 font-black">
                                <img src={coinLogo(c)} alt="" className="h-4 w-4 rounded-full" />
                                {w.coin}
                              </span>
                            </td>
                            <td className="py-2.5 px-3 text-right font-mono font-bold text-ink">
                              {formatAmount(w.amount, c)}
                            </td>
                            <td className="py-2.5 px-3 text-center">
                              <span
                                className={`inline-flex rounded-md px-2 py-0.5 text-[10px] font-black uppercase ${
                                  ok
                                    ? 'bg-emerald-500/10 text-emerald-700'
                                    : fail
                                      ? 'bg-rose-500/10 text-rose-700'
                                      : pend
                                        ? 'bg-amber-500/10 text-amber-700'
                                        : 'bg-surface text-ink-muted'
                                }`}
                              >
                                {ok ? 'ok' : fail ? 'fail' : pend ? 'pendente' : w.status}
                              </span>
                            </td>
                            <td className="py-2.5 px-4 text-right">
                              {w.tx_hash ? (
                                <span className="inline-flex items-center justify-end gap-1">
                                  <button
                                    type="button"
                                    onClick={() => copy(w.tx_hash!)}
                                    className="rounded-lg border border-border px-2 py-0.5 font-mono text-[11px] hover:bg-surface"
                                  >
                                    {shortHash(w.tx_hash)}
                                  </button>
                                  {url && (
                                    <a href={url} target="_blank" rel="noreferrer" className="text-bitcoin p-1">
                                      <i className="bi bi-box-arrow-up-right text-[10px]" />
                                    </a>
                                  )}
                                </span>
                              ) : (
                                <span className="text-ink-muted italic">—</span>
                              )}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                ))}
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

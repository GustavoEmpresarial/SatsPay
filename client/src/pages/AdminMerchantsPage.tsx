import { useMemo, useState, type ReactNode } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { api } from '../lib/api.js';
import { formatAdminAmount, formatAdminDate } from '../lib/admin.js';
import { coinLogo } from '../lib/coinAssets.js';
import { formatAmount, isCoin, safeBigInt, type Coin } from '@/shared';

interface MerchantItem {
  id: string;
  user_id: string;
  email: string;
  name: string;
  website_url?: string;
  webhook_url?: string;
  description?: string;
  status: string;
  is_verified: boolean;
  created_at: string;
  invoices_total: number;
  invoices_paid: number;
  volume_paid: string;
  fees_paid: string;
  api_keys_count: number;
  last_invoice_at?: string | null;
}

interface InvoiceWindow {
  created: number;
  paid: number;
  pending: number;
  expired: number;
}

interface CoinVolume {
  coin: string;
  paid_count: number;
  amount: string;
  fee_amount: string;
  net_amount: string;
}

interface TopMerchant {
  merchant_id: string;
  email: string;
  name: string;
  paid_count: number;
  volume_paid: string;
  fees_paid: string;
}

interface DayBucket {
  day: string;
  created: number;
  paid: number;
  expired: number;
}

interface RecentInvoice {
  id: string;
  merchant_id: string;
  merchant_email: string;
  merchant_name: string;
  coin: string;
  amount: string;
  fee_amount: string;
  status: string;
  order_id: string;
  site_name?: string | null;
  webhook_delivered: boolean;
  created_at: string;
  paid_at?: string | null;
}

interface MerchantPlatformStats {
  accounts_total: number;
  accounts_approved: number;
  accounts_pending: number;
  accounts_rejected: number;
  invoices_all: InvoiceWindow;
  invoices_24h: InvoiceWindow;
  invoices_7d: InvoiceWindow;
  invoices_30d: InvoiceWindow;
  conversion_pct: number;
  conversion_24h_pct: number;
  conversion_7d_pct: number;
  merchants_active_30d: number;
  merchants_new_7d: number;
  merchants_new_30d: number;
  webhook_success_pct: number;
  avg_confirm_minutes: number | null;
  volume_by_coin: CoinVolume[];
  volume_by_coin_30d: CoinVolume[];
  api_keys_active: number;
  api_keys_used_7d: number;
  webhooks_delivered: number;
  webhooks_failed: number;
  top_merchants: TopMerchant[];
  series_14d: DayBucket[];
  recent_invoices: RecentInvoice[];
}

interface EconCoinFlow {
  coin: string;
  count: number;
  volume: string;
  fees: string;
}

interface PlatformEconomics {
  all_time: {
    faucet_claims: number;
    faucet_by_coin: EconCoinFlow[];
    gateway_paid: number;
    gateway_by_coin: EconCoinFlow[];
    deposits_count: number;
    withdrawals_count: number;
  };
}

type Tab = 'overview' | 'stats' | 'merchants';
type StatsPeriod = '24h' | '7d' | '30d' | 'all';
type StatusFilter = 'ALL' | 'APPROVED' | 'PENDING' | 'REJECTED';

const EMPTY_WINDOW: InvoiceWindow = { created: 0, paid: 0, pending: 0, expired: 0 };

function windowForPeriod(stats: MerchantPlatformStats | undefined, period: StatsPeriod): InvoiceWindow {
  if (!stats) return EMPTY_WINDOW;
  if (period === '24h') return stats.invoices_24h ?? EMPTY_WINDOW;
  if (period === '7d') return stats.invoices_7d ?? EMPTY_WINDOW;
  if (period === '30d') return stats.invoices_30d ?? EMPTY_WINDOW;
  return stats.invoices_all ?? EMPTY_WINDOW;
}

function conversionForPeriod(stats: MerchantPlatformStats | undefined, period: StatsPeriod): number {
  if (!stats) return 0;
  if (period === '24h') return stats.conversion_24h_pct ?? 0;
  if (period === '7d') return stats.conversion_7d_pct ?? 0;
  if (period === '30d') {
    const w = stats.invoices_30d ?? EMPTY_WINDOW;
    return w.created > 0 ? (w.paid * 100) / w.created : 0;
  }
  return stats.conversion_pct ?? 0;
}

function invoiceStatusLabel(status: string): string {
  if (status === 'CONFIRMED') return 'Paga';
  if (status === 'PENDING') return 'Pendente';
  if (status === 'DETECTED') return 'Detectada';
  if (status === 'EXPIRED') return 'Expirada';
  if (status === 'CANCELLED') return 'Cancelada';
  return status;
}

function Kpi({
  label,
  value,
  hint,
  tone = 'default',
  icon,
}: {
  label: string;
  value: string | number;
  hint?: string;
  tone?: 'default' | 'good' | 'warn' | 'bad' | 'info';
  icon: string;
}) {
  const valueCls =
    tone === 'good'
      ? 'text-emerald-600'
      : tone === 'warn'
        ? 'text-amber-600'
        : tone === 'bad'
          ? 'text-rose-600'
          : tone === 'info'
            ? 'text-sky-600'
            : 'text-ink';
  const iconCls =
    tone === 'good'
      ? 'bg-emerald-500/10 text-emerald-600'
      : tone === 'warn'
        ? 'bg-amber-500/10 text-amber-600'
        : tone === 'bad'
          ? 'bg-rose-500/10 text-rose-600'
          : tone === 'info'
            ? 'bg-sky-500/10 text-sky-600'
            : 'bg-bitcoin/10 text-bitcoin';

  return (
    <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
      <div className="flex items-center justify-between text-ink-muted">
        <span className="text-[11px] font-bold uppercase tracking-wider">{label}</span>
        <div className={`flex h-7 w-7 items-center justify-center rounded-lg text-sm ${iconCls}`}>
          <i className={`bi ${icon}`} />
        </div>
      </div>
      <p className={`mt-2 text-2xl font-black tabular-nums ${valueCls}`}>{value}</p>
      {hint ? <span className="text-[10px] text-ink-muted font-medium">{hint}</span> : null}
    </div>
  );
}

function WindowCard({ title, w }: { title: string; w: InvoiceWindow }) {
  return (
    <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
      <h3 className="text-xs font-black uppercase tracking-wider text-ink-muted">{title}</h3>
      <div className="grid grid-cols-2 gap-3 text-sm">
        <div>
          <div className="text-[10px] font-bold text-ink-muted uppercase">Criadas</div>
          <div className="font-black tabular-nums text-ink">{w.created}</div>
        </div>
        <div>
          <div className="text-[10px] font-bold text-ink-muted uppercase">Pagas</div>
          <div className="font-black tabular-nums text-emerald-600">{w.paid}</div>
        </div>
        <div>
          <div className="text-[10px] font-bold text-ink-muted uppercase">Pendentes</div>
          <div className="font-black tabular-nums text-amber-600">{w.pending}</div>
        </div>
        <div>
          <div className="text-[10px] font-bold text-ink-muted uppercase">Expiradas</div>
          <div className="font-black tabular-nums text-rose-600">{w.expired}</div>
        </div>
      </div>
    </div>
  );
}

function ledgerDisplay(amount: string, coin?: string): string {
  if (coin && isCoin(coin)) return formatAdminAmount(amount, coin);
  try {
    const n = BigInt(amount || '0');
    if (n === 0n) return '0';
    return n.toLocaleString('pt-BR');
  } catch {
    return amount || '0';
  }
}

function statusFlags(m: MerchantItem) {
  const isApproved = m.status === 'APPROVED' || m.is_verified;
  const isPending = m.status === 'PENDING';
  const isSuspended = m.status === 'REJECTED' || (!m.is_verified && !isPending);
  return { isApproved, isPending, isSuspended };
}

export function AdminMerchantsPage() {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<Tab>('overview');
  const [statsPeriod, setStatsPeriod] = useState<StatsPeriod>('7d');
  const [searchTerm, setSearchTerm] = useState('');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('ALL');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const statsQ = useQuery<MerchantPlatformStats>({
    queryKey: ['admin-merchants-stats'],
    queryFn: () => api<MerchantPlatformStats>('/admin/merchants/stats'),
    refetchInterval: 15_000,
  });

  const econQ = useQuery<PlatformEconomics>({
    queryKey: ['admin-economics'],
    queryFn: () => api<PlatformEconomics>('/admin/economics'),
    refetchInterval: 15_000,
    enabled: tab === 'overview',
  });

  const { data: merchantsData, isLoading, isFetching, refetch } = useQuery<{ merchants: MerchantItem[] }>({
    queryKey: ['admin-merchants'],
    queryFn: () => api<{ merchants: MerchantItem[] }>('/admin/merchants'),
    refetchInterval: 15_000,
  });

  const approveMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/merchants/${id}/approve`, { method: 'POST' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-merchants'] });
      queryClient.invalidateQueries({ queryKey: ['admin-merchants-stats'] });
      queryClient.invalidateQueries({ queryKey: ['admin-economics'] });
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] });
    },
  });

  const suspendMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/merchants/${id}/suspend`, { method: 'POST' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-merchants'] });
      queryClient.invalidateQueries({ queryKey: ['admin-merchants-stats'] });
      queryClient.invalidateQueries({ queryKey: ['admin-economics'] });
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] });
    },
  });

  const merchants = merchantsData?.merchants ?? [];
  const stats = statsQ.data;

  const accountCounts = useMemo(() => {
    if (stats) {
      return {
        total: stats.accounts_total,
        approved: stats.accounts_approved,
        pending: stats.accounts_pending,
        suspended: stats.accounts_rejected,
      };
    }
    const total = merchants.length;
    const approved = merchants.filter((m) => m.status === 'APPROVED' || m.is_verified).length;
    const pending = merchants.filter((m) => m.status === 'PENDING').length;
    const suspended = merchants.filter(
      (m) => m.status === 'REJECTED' || (!m.is_verified && m.status !== 'PENDING'),
    ).length;
    return { total, approved, pending, suspended };
  }, [stats, merchants]);

  const filteredMerchants = useMemo(() => {
    return merchants.filter((m) => {
      const { isApproved, isPending, isSuspended } = statusFlags(m);
      const matchStatus =
        statusFilter === 'ALL' ||
        (statusFilter === 'APPROVED' && isApproved) ||
        (statusFilter === 'PENDING' && isPending) ||
        (statusFilter === 'REJECTED' && isSuspended);

      const matchSearch =
        !searchTerm.trim() ||
        m.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        m.email.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (m.website_url && m.website_url.toLowerCase().includes(searchTerm.toLowerCase())) ||
        (m.description && m.description.toLowerCase().includes(searchTerm.toLowerCase()));

      return matchStatus && matchSearch;
    });
  }, [merchants, statusFilter, searchTerm]);

  const refreshAll = () => {
    refetch();
    void statsQ.refetch();
    void econQ.refetch();
  };

  const tabs: { id: Tab; label: string }[] = [
    { id: 'overview', label: 'Visão geral' },
    { id: 'stats', label: 'Estatísticas' },
    { id: 'merchants', label: 'Comerciantes' },
  ];

  const periodWindow = windowForPeriod(stats, statsPeriod);
  const periodConversion = conversionForPeriod(stats, statsPeriod);
  const series = stats?.series_14d ?? [];
  const seriesMax = Math.max(1, ...series.map((d) => Math.max(d.created, d.paid, d.expired)));
  const funnelMax = Math.max(
    1,
    periodWindow.created,
    periodWindow.paid,
    periodWindow.pending,
    periodWindow.expired,
  );
  const coinVolume =
    statsPeriod === 'all'
      ? (stats?.volume_by_coin ?? [])
      : (stats?.volume_by_coin_30d ?? stats?.volume_by_coin ?? []);

  return (
    <div className="space-y-6 pb-16">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full border border-bitcoin/30 bg-bitcoin/10 px-3 py-1 text-xs font-black text-bitcoin">
            <i className="bi bi-shop" /> Gateway merchant
          </div>
          <h1 className="mt-1 text-2xl sm:text-3xl font-black text-ink tracking-tight">Comerciantes</h1>
          <p className="text-xs sm:text-sm text-ink-muted">
            Estatísticas do gateway e moderação de contas comerciais.
          </p>
        </div>

        <button
          type="button"
          onClick={refreshAll}
          className="inline-flex items-center gap-2 self-start sm:self-auto rounded-xl border border-border bg-paper px-4 py-2.5 text-xs font-bold text-ink shadow-xs hover:bg-surface transition"
        >
          <i className={`bi bi-arrow-repeat ${isFetching || statsQ.isFetching || econQ.isFetching ? 'animate-spin text-bitcoin' : ''}`} />
          <span>{isFetching || statsQ.isFetching || econQ.isFetching ? 'Atualizando...' : 'Atualizar'}</span>
        </button>
      </div>

      <div className="flex flex-wrap gap-2 border-b border-border pb-1">
        {tabs.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            className={`rounded-t-xl px-4 py-2 text-xs font-black transition ${
              tab === t.id
                ? 'bg-paper text-ink border border-border border-b-paper -mb-px shadow-xs'
                : 'text-ink-muted hover:text-ink'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'overview' && (
        <div className="space-y-6">
          {statsQ.isLoading && !stats ? (
            <div className="py-16 text-center text-ink-muted text-xs animate-pulse">Carregando estatísticas...</div>
          ) : (
            <>
              <div className="rounded-2xl border border-border bg-gradient-to-br from-bitcoin/10 via-paper to-emerald-500/5 p-4 shadow-xs">
                <div className="flex flex-wrap items-start justify-between gap-3 mb-3">
                  <div>
                    <h2 className="text-sm font-black text-ink">Dinheiro em movimento</h2>
                    <p className="text-[11px] text-ink-muted">
                      Gateway = receita (~0,5%) · Faucet = custo HOUSE. Detalhe completo na Visão geral → Economia.
                    </p>
                  </div>
                  <Link
                    to="/admin"
                    className="text-[11px] font-black text-bitcoin hover:underline whitespace-nowrap"
                  >
                    Economia completa →
                  </Link>
                </div>
                <div className="grid sm:grid-cols-2 gap-3">
                  <div className="rounded-xl border border-emerald-500/25 bg-emerald-500/5 p-3">
                    <div className="text-[10px] font-black uppercase tracking-wider text-emerald-800">
                      Receita gateway (histórico)
                    </div>
                    {(econQ.data?.all_time.gateway_by_coin ?? []).filter((r) => safeBigInt(r.fees) > 0n).length ===
                    0 ? (
                      <p className="mt-2 text-xs text-ink-muted">
                        Ainda sem invoices confirmadas — taxas aparecem aqui quando o gateway processar.
                      </p>
                    ) : (
                      <ul className="mt-2 space-y-1.5">
                        {econQ.data!.all_time.gateway_by_coin
                          .filter((r) => safeBigInt(r.fees) > 0n)
                          .map((r) => {
                            const coin = isCoin(r.coin) ? (r.coin as Coin) : null;
                            return (
                              <li key={r.coin} className="flex items-center justify-between text-xs gap-2">
                                <span className="inline-flex items-center gap-1.5 font-bold">
                                  {coin ? <img src={coinLogo(coin)} alt="" className="h-4 w-4 rounded-full" /> : null}
                                  {r.coin}
                                </span>
                                <span className="font-mono font-black text-emerald-700">
                                  +{coin ? formatAmount(r.fees, coin) : r.fees}
                                </span>
                              </li>
                            );
                          })}
                      </ul>
                    )}
                  </div>
                  <div className="rounded-xl border border-rose-500/25 bg-rose-500/5 p-3">
                    <div className="text-[10px] font-black uppercase tracking-wider text-rose-800">
                      Custo faucet (histórico)
                    </div>
                    {(econQ.data?.all_time.faucet_by_coin ?? []).filter((r) => safeBigInt(r.volume) > 0n)
                      .length === 0 ? (
                      <p className="mt-2 text-xs text-ink-muted">
                        Sem resgates — faucet debitaria a HOUSE (custo, não receita).
                      </p>
                    ) : (
                      <ul className="mt-2 space-y-1.5">
                        {econQ.data!.all_time.faucet_by_coin
                          .filter((r) => safeBigInt(r.volume) > 0n)
                          .map((r) => {
                            const coin = isCoin(r.coin) ? (r.coin as Coin) : null;
                            return (
                              <li key={r.coin} className="flex items-center justify-between text-xs gap-2">
                                <span className="inline-flex items-center gap-1.5 font-bold">
                                  {coin ? <img src={coinLogo(coin)} alt="" className="h-4 w-4 rounded-full" /> : null}
                                  {r.coin} · {r.count}×
                                </span>
                                <span className="font-mono font-black text-rose-700">
                                  −{coin ? formatAmount(r.volume, coin) : r.volume}
                                </span>
                              </li>
                            );
                          })}
                      </ul>
                    )}
                  </div>
                </div>
              </div>

              <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                <Kpi label="Contas" value={accountCounts.total} hint="Com merchant_status ≠ NONE" icon="bi-building" />
                <Kpi
                  label="Aprovados"
                  value={accountCounts.approved}
                  tone="good"
                  hint="Prontos para cobrar"
                  icon="bi-patch-check-fill"
                />
                <Kpi
                  label="Pendentes"
                  value={accountCounts.pending}
                  tone="warn"
                  hint="Aguardando revisão"
                  icon="bi-hourglass-split"
                />
                <Kpi
                  label="Rejeitados"
                  value={accountCounts.suspended}
                  tone="bad"
                  hint="Sem checkout"
                  icon="bi-slash-circle"
                />
              </div>

              <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                <Kpi
                  label="Conversão"
                  value={`${(stats?.conversion_pct ?? 0).toFixed(1)}%`}
                  tone="info"
                  hint="Pagas / criadas (histórico)"
                  icon="bi-graph-up-arrow"
                />
                <Kpi
                  label="Ativos 30d"
                  value={stats?.merchants_active_30d ?? 0}
                  tone="good"
                  hint="Com fatura paga no período"
                  icon="bi-lightning-charge"
                />
                <Kpi
                  label="Webhooks ok"
                  value={`${(stats?.webhook_success_pct ?? 100).toFixed(0)}%`}
                  tone={stats && stats.webhooks_failed > 0 ? 'warn' : 'good'}
                  icon="bi-check2-circle"
                  hint={`${stats?.webhooks_delivered ?? 0} entregues · ${stats?.webhooks_failed ?? 0} falhos`}
                />
                <Kpi
                  label="Faturas 24h"
                  value={stats?.invoices_24h?.created ?? 0}
                  icon="bi-receipt"
                  hint={`${stats?.invoices_24h?.paid ?? 0} pagas`}
                />
              </div>

              <div className="rounded-2xl border border-dashed border-border bg-surface/40 p-5 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
                <div>
                  <h3 className="text-sm font-black text-ink">Quer o detalhe completo?</h3>
                  <p className="text-[11px] text-ink-muted mt-0.5">
                    Funil, série 14 dias, volume por moeda, top comerciantes e faturas recentes.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => setTab('stats')}
                  className="inline-flex items-center gap-2 self-start rounded-xl bg-bitcoin px-4 py-2.5 text-xs font-black text-white shadow-xs hover:opacity-90"
                >
                  <i className="bi bi-bar-chart-line" />
                  Abrir estatísticas
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {tab === 'stats' && (
        <div className="space-y-6">
          {statsQ.isLoading && !stats ? (
            <div className="py-16 text-center text-ink-muted text-xs animate-pulse">Carregando estatísticas...</div>
          ) : (
            <>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <h2 className="text-sm font-black text-ink">Estatísticas do gateway</h2>
                  <p className="text-[11px] text-ink-muted">
                    Só comerciantes — faturas, conversão, webhooks e ranking.
                  </p>
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {(
                    [
                      ['24h', '24h'],
                      ['7d', '7 dias'],
                      ['30d', '30 dias'],
                      ['all', 'Histórico'],
                    ] as const
                  ).map(([key, label]) => (
                    <button
                      key={key}
                      type="button"
                      onClick={() => setStatsPeriod(key)}
                      className={`rounded-xl px-3 py-1.5 text-[11px] font-black transition ${
                        statsPeriod === key
                          ? 'bg-ink text-paper shadow-xs'
                          : 'border border-border bg-paper text-ink-muted hover:text-ink'
                      }`}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                <Kpi
                  label="Criadas"
                  value={periodWindow.created}
                  icon="bi-file-earmark-plus"
                  hint={statsPeriod === 'all' ? 'Histórico' : `Janela ${statsPeriod}`}
                />
                <Kpi
                  label="Pagas"
                  value={periodWindow.paid}
                  tone="good"
                  icon="bi-check2-circle"
                  hint={`${periodConversion.toFixed(1)}% conversão`}
                />
                <Kpi
                  label="Pendentes"
                  value={periodWindow.pending}
                  tone="warn"
                  icon="bi-hourglass-split"
                  hint="Aguardando pagamento"
                />
                <Kpi
                  label="Expiradas"
                  value={periodWindow.expired}
                  tone="bad"
                  icon="bi-x-octagon"
                  hint="Expiradas / canceladas"
                />
              </div>

              <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                <Kpi
                  label="Comerciantes ativos"
                  value={stats?.merchants_active_30d ?? 0}
                  tone="info"
                  icon="bi-shop"
                  hint="Pagaram nos últimos 30d"
                />
                <Kpi
                  label="Novos 7d"
                  value={stats?.merchants_new_7d ?? 0}
                  icon="bi-person-plus"
                  hint={`${stats?.merchants_new_30d ?? 0} nos últimos 30d`}
                />
                <Kpi
                  label="Webhook success"
                  value={`${(stats?.webhook_success_pct ?? 100).toFixed(1)}%`}
                  tone={stats && stats.webhooks_failed > 0 ? 'warn' : 'good'}
                  icon="bi-broadcast"
                  hint={`${stats?.webhooks_delivered ?? 0} ok · ${stats?.webhooks_failed ?? 0} falhos`}
                />
                <Kpi
                  label="Tempo médio até pagar"
                  value={
                    stats?.avg_confirm_minutes != null
                      ? stats.avg_confirm_minutes < 60
                        ? `${stats.avg_confirm_minutes.toFixed(0)} min`
                        : `${(stats.avg_confirm_minutes / 60).toFixed(1)} h`
                      : '—'
                  }
                  icon="bi-stopwatch"
                  hint="paid_at − created_at"
                />
              </div>

              <div className="grid lg:grid-cols-2 gap-4">
                <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-4">
                  <div>
                    <h3 className="text-sm font-black text-ink">Funil de faturas</h3>
                    <p className="text-[11px] text-ink-muted">Distribuição no período selecionado</p>
                  </div>
                  {(
                    [
                      ['Criadas', periodWindow.created, 'bg-ink'],
                      ['Pagas', periodWindow.paid, 'bg-emerald-500'],
                      ['Pendentes', periodWindow.pending, 'bg-amber-500'],
                      ['Expiradas', periodWindow.expired, 'bg-rose-500'],
                    ] as const
                  ).map(([label, value, bar]) => (
                    <div key={label} className="space-y-1">
                      <div className="flex justify-between text-[11px] font-bold">
                        <span className="text-ink-muted">{label}</span>
                        <span className="font-mono tabular-nums text-ink">{value}</span>
                      </div>
                      <div className="h-2 rounded-full bg-surface overflow-hidden">
                        <div
                          className={`h-full rounded-full ${bar}`}
                          style={{ width: `${Math.max(value > 0 ? 4 : 0, (value / funnelMax) * 100)}%` }}
                        />
                      </div>
                    </div>
                  ))}
                </div>

                <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div>
                      <h3 className="text-sm font-black text-ink">Atividade 14 dias</h3>
                      <p className="text-[11px] text-ink-muted">Criadas × pagas × expiradas</p>
                    </div>
                    <div className="flex items-center gap-3 text-[10px] font-bold text-ink-muted">
                      <span className="inline-flex items-center gap-1">
                        <span className="h-2 w-2 rounded-sm bg-ink/70" /> Criadas
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <span className="h-2 w-2 rounded-sm bg-emerald-500" /> Pagas
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <span className="h-2 w-2 rounded-sm bg-rose-400" /> Exp.
                      </span>
                    </div>
                  </div>
                  {series.length === 0 ? (
                    <p className="py-10 text-center text-xs text-ink-muted">Sem faturas nos últimos 14 dias.</p>
                  ) : (
                    <div className="flex items-end gap-1 sm:gap-1.5 min-h-[120px] pt-2 overflow-x-auto">
                      {series.map((d) => (
                        <div key={d.day} className="flex-1 min-w-[18px] flex flex-col items-center gap-1">
                          <div className="flex items-end gap-0.5 h-24 w-full justify-center">
                            <div
                              className="w-1 sm:w-1.5 rounded-t bg-ink/60"
                              style={{
                                height: `${Math.max(d.created > 0 ? 6 : 0, (d.created / seriesMax) * 100)}%`,
                              }}
                              title={`${d.day}: ${d.created} criadas`}
                            />
                            <div
                              className="w-1 sm:w-1.5 rounded-t bg-emerald-500"
                              style={{
                                height: `${Math.max(d.paid > 0 ? 6 : 0, (d.paid / seriesMax) * 100)}%`,
                              }}
                              title={`${d.day}: ${d.paid} pagas`}
                            />
                            <div
                              className="w-1 sm:w-1.5 rounded-t bg-rose-400"
                              style={{
                                height: `${Math.max(d.expired > 0 ? 6 : 0, (d.expired / seriesMax) * 100)}%`,
                              }}
                              title={`${d.day}: ${d.expired} expiradas`}
                            />
                          </div>
                          <span className="text-[9px] font-mono text-ink-muted">
                            {d.day.slice(5)}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>

              <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-4">
                <WindowCard title="Faturas — 24h" w={stats?.invoices_24h ?? EMPTY_WINDOW} />
                <WindowCard title="Faturas — 7 dias" w={stats?.invoices_7d ?? EMPTY_WINDOW} />
                <WindowCard title="Faturas — 30 dias" w={stats?.invoices_30d ?? EMPTY_WINDOW} />
                <WindowCard title="Faturas — histórico" w={stats?.invoices_all ?? EMPTY_WINDOW} />
              </div>

              <div className="grid lg:grid-cols-2 gap-4">
                <div className="rounded-2xl border border-border bg-paper shadow-xs overflow-hidden">
                  <div className="border-b border-border px-4 py-3">
                    <h3 className="text-sm font-black text-ink">Volume pago por moeda</h3>
                    <p className="text-[11px] text-ink-muted">
                      {statsPeriod === 'all' ? 'Histórico completo' : 'Últimos 30 dias'} · unidades nativas
                    </p>
                  </div>
                  {coinVolume.length === 0 ? (
                    <div className="py-10 text-center text-xs text-ink-muted">Nenhuma invoice confirmada ainda.</div>
                  ) : (
                    <div className="overflow-x-auto">
                      <table className="w-full text-left text-xs">
                        <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                          <tr>
                            <th className="p-3">Moeda</th>
                            <th className="p-3">Pagas</th>
                            <th className="p-3">Volume</th>
                            <th className="p-3">Taxas</th>
                            <th className="p-3">Líquido</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-border">
                          {coinVolume.map((row) => {
                            const coin = isCoin(row.coin) ? (row.coin as Coin) : null;
                            return (
                              <tr key={row.coin} className="hover:bg-surface/40">
                                <td className="p-3">
                                  <div className="flex items-center gap-2 font-black text-ink">
                                    {coin ? (
                                      <img src={coinLogo(coin)} alt="" className="h-5 w-5 rounded-full" />
                                    ) : null}
                                    {row.coin}
                                  </div>
                                </td>
                                <td className="p-3 tabular-nums font-mono">{row.paid_count}</td>
                                <td className="p-3 tabular-nums font-mono font-bold">
                                  {coin ? formatAmount(row.amount, coin) : row.amount}
                                </td>
                                <td className="p-3 tabular-nums font-mono text-emerald-700">
                                  {coin ? formatAmount(row.fee_amount, coin) : row.fee_amount}
                                </td>
                                <td className="p-3 tabular-nums font-mono text-ink-muted">
                                  {coin ? formatAmount(row.net_amount, coin) : row.net_amount}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>

                <div className="rounded-2xl border border-border bg-paper shadow-xs overflow-hidden">
                  <div className="border-b border-border px-4 py-3 flex items-center justify-between gap-2">
                    <div>
                      <h3 className="text-sm font-black text-ink">Top comerciantes</h3>
                      <p className="text-[11px] text-ink-muted">Por volume pago (histórico)</p>
                    </div>
                    <button
                      type="button"
                      onClick={() => setTab('merchants')}
                      className="text-[11px] font-bold text-bitcoin hover:underline"
                    >
                      Ver lista →
                    </button>
                  </div>
                  {(stats?.top_merchants?.length ?? 0) === 0 ? (
                    <div className="py-10 text-center text-xs text-ink-muted">Sem volume pago ainda.</div>
                  ) : (
                    <div className="overflow-x-auto">
                      <table className="w-full text-left text-xs">
                        <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                          <tr>
                            <th className="p-3">#</th>
                            <th className="p-3">Comerciante</th>
                            <th className="p-3">Pagas</th>
                            <th className="p-3">Volume</th>
                            <th className="p-3">Taxas</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-border">
                          {stats!.top_merchants.map((m, i) => (
                            <tr key={m.merchant_id} className="hover:bg-surface/40">
                              <td className="p-3 font-black text-ink-muted">{i + 1}</td>
                              <td className="p-3">
                                <div className="font-black text-ink">{m.name}</div>
                                <div className="font-mono text-[11px] text-ink-muted">{m.email}</div>
                              </td>
                              <td className="p-3 tabular-nums font-mono">{m.paid_count}</td>
                              <td className="p-3 tabular-nums font-mono font-bold">
                                {ledgerDisplay(m.volume_paid)}
                              </td>
                              <td className="p-3 tabular-nums font-mono text-emerald-700">
                                {ledgerDisplay(m.fees_paid)}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>
              </div>

              <div className="rounded-2xl border border-border bg-paper shadow-xs overflow-hidden">
                <div className="border-b border-border px-4 py-3">
                  <h3 className="text-sm font-black text-ink">Faturas recentes</h3>
                  <p className="text-[11px] text-ink-muted">Últimas 20 no gateway</p>
                </div>
                {(stats?.recent_invoices?.length ?? 0) === 0 ? (
                  <div className="py-10 text-center text-xs text-ink-muted">Nenhuma fatura ainda.</div>
                ) : (
                  <div className="overflow-x-auto">
                    <table className="w-full text-left text-xs">
                      <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                        <tr>
                          <th className="p-3">Quando</th>
                          <th className="p-3">Comerciante</th>
                          <th className="p-3">Pedido</th>
                          <th className="p-3">Moeda</th>
                          <th className="p-3">Valor</th>
                          <th className="p-3">Status</th>
                          <th className="p-3">Webhook</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-border">
                        {stats!.recent_invoices.map((inv) => {
                          const coin = isCoin(inv.coin) ? (inv.coin as Coin) : null;
                          return (
                            <tr key={inv.id} className="hover:bg-surface/40">
                              <td className="p-3 font-mono text-[11px] text-ink-muted whitespace-nowrap">
                                {formatAdminDate(inv.created_at)}
                              </td>
                              <td className="p-3">
                                <div className="font-bold text-ink">{inv.merchant_name}</div>
                                <div className="font-mono text-[10px] text-ink-muted">{inv.merchant_email}</div>
                              </td>
                              <td className="p-3 font-mono text-[11px]">
                                <div className="truncate max-w-[120px]" title={inv.order_id}>
                                  {inv.order_id}
                                </div>
                                {inv.site_name ? (
                                  <div className="text-[10px] text-ink-muted truncate">{inv.site_name}</div>
                                ) : null}
                              </td>
                              <td className="p-3">
                                <span className="inline-flex items-center gap-1.5 font-black">
                                  {coin ? (
                                    <img src={coinLogo(coin)} alt="" className="h-4 w-4 rounded-full" />
                                  ) : null}
                                  {inv.coin}
                                </span>
                              </td>
                              <td className="p-3 font-mono font-bold tabular-nums">
                                {coin ? formatAmount(inv.amount, coin) : inv.amount}
                              </td>
                              <td className="p-3">
                                <span
                                  className={`inline-flex rounded-full px-2 py-0.5 text-[10px] font-black ${
                                    inv.status === 'CONFIRMED'
                                      ? 'bg-emerald-500/10 text-emerald-700'
                                      : inv.status === 'PENDING' || inv.status === 'DETECTED'
                                        ? 'bg-amber-500/10 text-amber-700'
                                        : 'bg-rose-500/10 text-rose-700'
                                  }`}
                                >
                                  {invoiceStatusLabel(inv.status)}
                                </span>
                              </td>
                              <td className="p-3">
                                {inv.status !== 'CONFIRMED' ? (
                                  <span className="text-ink-muted">—</span>
                                ) : inv.webhook_delivered ? (
                                  <span className="text-emerald-700 font-bold">ok</span>
                                ) : (
                                  <span className="text-rose-700 font-bold">falhou</span>
                                )}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>

              <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                <Kpi
                  label="API keys ativas"
                  value={stats?.api_keys_active ?? 0}
                  icon="bi-key"
                  hint={`${stats?.api_keys_used_7d ?? 0} usadas em 7d`}
                />
                <Kpi
                  label="Conversão 24h"
                  value={`${(stats?.conversion_24h_pct ?? 0).toFixed(1)}%`}
                  tone="info"
                  icon="bi-graph-up"
                />
                <Kpi
                  label="Conversão 7d"
                  value={`${(stats?.conversion_7d_pct ?? 0).toFixed(1)}%`}
                  tone="info"
                  icon="bi-graph-up-arrow"
                />
                <Kpi
                  label="Conversão histórica"
                  value={`${(stats?.conversion_pct ?? 0).toFixed(1)}%`}
                  icon="bi-pie-chart"
                />
              </div>
            </>
          )}
        </div>
      )}

      {tab === 'merchants' && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
            <Kpi label="Total" value={accountCounts.total} icon="bi-building" />
            <Kpi label="Ativos" value={accountCounts.approved} tone="good" icon="bi-patch-check-fill" />
            <Kpi label="Pendentes" value={accountCounts.pending} tone="warn" icon="bi-hourglass-split" />
            <Kpi label="Suspensos" value={accountCounts.suspended} tone="bad" icon="bi-slash-circle" />
          </div>

          <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
            <div className="border-b border-border bg-surface/60 p-4 sm:p-5 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
              <div className="flex flex-wrap items-center gap-2">
                {(
                  [
                    ['ALL', `Todos (${accountCounts.total})`],
                    ['APPROVED', `Ativos (${accountCounts.approved})`],
                    ['PENDING', `Pendentes (${accountCounts.pending})`],
                    ['REJECTED', `Suspensos (${accountCounts.suspended})`],
                  ] as const
                ).map(([key, label]) => (
                  <button
                    key={key}
                    type="button"
                    onClick={() => setStatusFilter(key)}
                    className={`rounded-xl px-3 py-1.5 text-xs font-bold transition ${
                      statusFilter === key
                        ? key === 'APPROVED'
                          ? 'bg-emerald-600 text-white shadow-xs'
                          : key === 'PENDING'
                            ? 'bg-amber-600 text-white shadow-xs'
                            : key === 'REJECTED'
                              ? 'bg-rose-600 text-white shadow-xs'
                              : 'bg-ink text-paper shadow-xs'
                        : 'bg-surface text-ink-muted hover:text-ink border border-border'
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>

              <div className="relative">
                <i className="bi bi-search absolute left-3 top-1/2 -translate-y-1/2 text-ink-muted text-xs" />
                <input
                  type="text"
                  placeholder="Buscar por empresa, e-mail ou site..."
                  value={searchTerm}
                  onChange={(e) => setSearchTerm(e.target.value)}
                  className="w-full sm:w-64 rounded-xl border border-border bg-paper pl-8 pr-3 py-1.5 text-xs text-ink placeholder:text-ink-muted/60 focus:outline-none focus:ring-2 focus:ring-bitcoin/20"
                />
              </div>
            </div>

            {isLoading ? (
              <div className="py-16 text-center text-ink-muted text-xs animate-pulse">
                Carregando comerciantes...
              </div>
            ) : filteredMerchants.length === 0 ? (
              <div className="py-16 text-center space-y-3 px-4">
                <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-bitcoin/10 text-bitcoin text-2xl mx-auto">
                  <i className="bi bi-shop-window" />
                </div>
                <div>
                  <p className="text-sm font-bold text-ink">Nenhum comerciante encontrado</p>
                  <p className="text-xs text-ink-muted max-w-md mx-auto mt-1">
                    {searchTerm || statusFilter !== 'ALL'
                      ? 'Tente alterar busca ou filtros.'
                      : 'Novos cadastros comerciais aparecerão aqui.'}
                  </p>
                </div>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs">
                  <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                    <tr>
                      <th className="p-4">Empresa</th>
                      <th className="p-4">Faturas</th>
                      <th className="p-4">Volume pago</th>
                      <th className="p-4">Chaves</th>
                      <th className="p-4">Última invoice</th>
                      <th className="p-4">Status</th>
                      <th className="p-4 text-right">Ações</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {filteredMerchants.map((m) => {
                      const { isApproved, isPending, isSuspended } = statusFlags(m);
                      const initials = (m.name || m.email || 'M').substring(0, 2).toUpperCase();
                      const open = expandedId === m.id;

                      return (
                        <MerchantRow
                          key={m.id}
                          m={m}
                          initials={initials}
                          isApproved={isApproved}
                          isPending={isPending}
                          isSuspended={isSuspended}
                          open={open}
                          onToggle={() => setExpandedId(open ? null : m.id)}
                          onApprove={() => approveMutation.mutate(m.id)}
                          onSuspend={() => suspendMutation.mutate(m.id)}
                          approvePending={approveMutation.isPending}
                          suspendPending={suspendMutation.isPending}
                        />
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function MerchantRow({
  m,
  initials,
  isApproved,
  isPending,
  isSuspended,
  open,
  onToggle,
  onApprove,
  onSuspend,
  approvePending,
  suspendPending,
}: {
  m: MerchantItem;
  initials: string;
  isApproved: boolean;
  isPending: boolean;
  isSuspended: boolean;
  open: boolean;
  onToggle: () => void;
  onApprove: () => void;
  onSuspend: () => void;
  approvePending: boolean;
  suspendPending: boolean;
}) {
  const detail: ReactNode = open ? (
    <tr className="bg-surface/30">
      <td colSpan={7} className="p-4">
        <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-3 text-[11px]">
          <div>
            <div className="font-bold text-ink-muted uppercase text-[10px]">Site</div>
            {m.website_url ? (
              <a
                href={m.website_url.startsWith('http') ? m.website_url : `https://${m.website_url}`}
                target="_blank"
                rel="noreferrer"
                className="text-bitcoin hover:underline font-mono font-bold"
              >
                {m.website_url.replace(/^https?:\/\//, '')}
              </a>
            ) : (
              <span className="italic text-ink-muted">Não informado</span>
            )}
          </div>
          <div>
            <div className="font-bold text-ink-muted uppercase text-[10px]">Cadastro</div>
            <div className="font-mono">{formatAdminDate(m.created_at)}</div>
          </div>
          <div>
            <div className="font-bold text-ink-muted uppercase text-[10px]">Taxas pagas (ledger)</div>
            <div className="font-mono">{ledgerDisplay(m.fees_paid ?? '0')}</div>
          </div>
          <div className="sm:col-span-2 lg:col-span-3">
            <div className="font-bold text-ink-muted uppercase text-[10px]">Descrição</div>
            <p className="text-ink-muted">{m.description || 'Sem descrição'}</p>
          </div>
        </div>
      </td>
    </tr>
  ) : null;

  return (
    <>
      <tr className="hover:bg-surface/40 transition">
        <td className="p-4">
          <button type="button" onClick={onToggle} className="flex items-center gap-3 text-left w-full">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-bitcoin/10 font-black text-bitcoin text-xs">
              {initials}
            </div>
            <div>
              <div className="font-black text-ink text-sm leading-tight flex items-center gap-1.5">
                <span>{m.name}</span>
                <i className={`bi bi-chevron-${open ? 'up' : 'down'} text-[10px] text-ink-muted`} />
              </div>
              <span className="font-mono text-[11px] text-ink-muted block mt-0.5">{m.email}</span>
            </div>
          </button>
        </td>
        <td className="p-4 font-mono tabular-nums">
          <span className="font-bold text-emerald-700">{m.invoices_paid ?? 0}</span>
          <span className="text-ink-muted"> / {m.invoices_total ?? 0}</span>
        </td>
        <td className="p-4 font-mono tabular-nums font-bold">{ledgerDisplay(m.volume_paid ?? '0')}</td>
        <td className="p-4 font-mono tabular-nums">{m.api_keys_count ?? 0}</td>
        <td className="p-4 font-mono text-[11px] text-ink-muted whitespace-nowrap">
          {formatAdminDate(m.last_invoice_at)}
        </td>
        <td className="p-4 whitespace-nowrap">
          {isApproved ? (
            <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/10 border border-emerald-500/30 px-2.5 py-1 text-[10px] font-black text-emerald-700">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
              Aprovado
            </span>
          ) : isPending ? (
            <span className="inline-flex items-center gap-1.5 rounded-full bg-amber-500/15 border border-amber-500/30 px-2.5 py-1 text-[10px] font-black text-amber-700">
              <i className="bi bi-clock-history" />
              Pendente
            </span>
          ) : (
            <span className="inline-flex items-center gap-1.5 rounded-full bg-rose-500/10 border border-rose-500/30 px-2.5 py-1 text-[10px] font-black text-rose-700">
              <i className="bi bi-slash-circle" />
              Suspenso
            </span>
          )}
        </td>
        <td className="p-4 text-right whitespace-nowrap">
          <div className="flex items-center justify-end gap-2">
            {!isApproved && (
              <button
                type="button"
                onClick={onApprove}
                disabled={approvePending}
                className="inline-flex items-center gap-1.5 rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white px-3 py-1.5 text-xs font-black transition active:scale-95 disabled:opacity-50 shadow-xs"
              >
                <i className="bi bi-check-lg" />
                <span>Aprovar</span>
              </button>
            )}
            {!isSuspended && (
              <button
                type="button"
                onClick={onSuspend}
                disabled={suspendPending}
                className="inline-flex items-center gap-1.5 rounded-xl bg-rose-500/10 hover:bg-rose-500/20 text-rose-700 px-3 py-1.5 text-xs font-bold transition active:scale-95 disabled:opacity-50"
              >
                <i className="bi bi-x-circle" />
                <span>Suspender</span>
              </button>
            )}
          </div>
        </td>
      </tr>
      {detail}
    </>
  );
}

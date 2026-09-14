import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { clsx } from 'clsx';
import { api } from '../lib/api.js';
import { formatAdminDate } from '../lib/admin.js';
import {
  classifyError,
  isExpectedAuthNoise,
  redactSecrets,
  type ClassifiedError,
} from '../lib/errorConsole.js';

type Tab = 'saude' | 'apm' | 'erros';

interface BreadcrumbItem {
  category: string;
  message: string;
  timestamp: string;
  data?: Record<string, unknown>;
}

interface SystemErrorLog {
  id: string;
  fingerprint: string;
  service: string;
  level: string;
  message: string;
  stack_trace?: string;
  endpoint?: string;
  method?: string;
  status_code?: number;
  user_id?: string;
  ip_address?: string;
  request_payload?: {
    breadcrumbs?: BreadcrumbItem[];
    [key: string]: unknown;
  };
  occurrences_count: number;
  status: 'OPEN' | 'INVESTIGATING' | 'RESOLVED' | 'IGNORED';
  first_seen_at: string;
  last_seen_at: string;
}

interface TelemetryOverview {
  total_errors_24h: number;
  open_errors_count: number;
  critical_errors_count: number;
  active_db_connections: number;
  total_users: number;
  total_wallets: number;
  total_merchants: number;
  pending_withdrawals: number;
  avg_latency_ms: number;
  system_health_pct: number;
}

interface SystemMetricsSnapshot {
  id: string;
  rpm: number;
  avg_latency_ms: number;
  p95_latency_ms: number;
  error_rate_pct: string | number;
  active_db_connections: number;
  captured_at: string;
}

interface AdminDashboardStats {
  server: {
    memory_used_mb: number;
    memory_pct: number;
    db_connections_active: number;
    db_connections_max: number;
    cpu_load_1m: number;
    status: string;
  };
}

function isNoiseError(err: SystemErrorLog): boolean {
  const m = err.message.toLowerCase();
  return (
    m.includes('failed to load <img>') ||
    m.includes('failed to load img') ||
    (err.level === 'WARN' && (m.includes('.svg') || m.includes('logo.png'))) ||
    isExpectedAuthNoise({
      message: err.message,
      status_code: err.status_code,
      endpoint: err.endpoint,
    })
  );
}

export function AdminTelemetryPage() {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<Tab>('saude');
  const [statusFilter, setStatusFilter] = useState('OPEN');
  const [serviceFilter, setServiceFilter] = useState('ALL');
  const [levelFilter, setLevelFilter] = useState('ALL');
  const [searchQuery, setSearchQuery] = useState('');
  const [hideNoise, setHideNoise] = useState(true);
  const [errorSort, setErrorSort] = useState<'recent' | 'frequent'>('recent');
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [autoMs, setAutoMs] = useState<number | false>(10_000);
  const [metricsHours, setMetricsHours] = useState(24);

  const overviewQ = useQuery<TelemetryOverview>({
    queryKey: ['admin-telemetry-overview'],
    queryFn: () => api<TelemetryOverview>('/admin/telemetry/overview'),
    refetchInterval: autoMs,
  });

  const metricsQ = useQuery<{ snapshots: SystemMetricsSnapshot[] }>({
    queryKey: ['admin-telemetry-metrics-history', metricsHours],
    queryFn: () =>
      api<{ snapshots: SystemMetricsSnapshot[] }>(
        `/admin/telemetry/metrics-history?hours=${metricsHours}`,
      ),
    refetchInterval: autoMs,
    enabled: tab === 'apm' || tab === 'saude',
  });

  const statsQ = useQuery<AdminDashboardStats>({
    queryKey: ['admin-stats'],
    queryFn: () => api<AdminDashboardStats>('/admin/stats'),
    refetchInterval: autoMs,
    enabled: tab === 'saude',
  });

  const errorsQ = useQuery<{ errors: SystemErrorLog[] }>({
    queryKey: ['admin-telemetry-errors', statusFilter, serviceFilter, levelFilter, searchQuery],
    queryFn: () => {
      const params = new URLSearchParams();
      if (statusFilter !== 'ALL') params.set('status', statusFilter);
      if (serviceFilter !== 'ALL') params.set('service', serviceFilter);
      if (levelFilter !== 'ALL') params.set('level', levelFilter);
      if (searchQuery.trim()) params.set('search', searchQuery.trim());
      return api<{ errors: SystemErrorLog[] }>(`/admin/telemetry/errors?${params.toString()}`);
    },
    refetchInterval: autoMs,
    enabled: tab === 'erros' || tab === 'saude',
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ['admin-telemetry-errors'] });
    void queryClient.invalidateQueries({ queryKey: ['admin-telemetry-overview'] });
    void queryClient.invalidateQueries({ queryKey: ['admin-telemetry-metrics-history'] });
  };

  const resolveMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/telemetry/errors/${id}/resolve`, { method: 'POST' }),
    onSuccess: invalidate,
  });
  const resolveAllMutation = useMutation({
    mutationFn: () => api('/admin/telemetry/errors/resolve-all', { method: 'POST' }),
    onSuccess: invalidate,
  });
  const ignoreMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/telemetry/errors/${id}/ignore`, { method: 'POST' }),
    onSuccess: invalidate,
  });
  const clearResolvedMutation = useMutation({
    mutationFn: () => api('/admin/telemetry/errors/clear', { method: 'POST' }),
    onSuccess: invalidate,
  });

  const overview = overviewQ.data;
  const server = statsQ.data?.server;
  const snapshots = metricsQ.data?.snapshots ?? [];
  const errors = useMemo(() => {
    const list = errorsQ.data?.errors ?? [];
    const filtered = hideNoise ? list.filter((e) => !isNoiseError(e)) : list;
    const sorted = [...filtered];
    if (errorSort === 'frequent') {
      sorted.sort((a, b) => b.occurrences_count - a.occurrences_count || b.last_seen_at.localeCompare(a.last_seen_at));
    } else {
      sorted.sort((a, b) => b.last_seen_at.localeCompare(a.last_seen_at));
    }
    return sorted;
  }, [errorsQ.data?.errors, hideNoise, errorSort]);

  const errorStats = useMemo(() => {
    const groups = errors.length;
    const groups5xx = errors.filter((e) => (e.status_code ?? 0) >= 500).length;
    return { groups, groups5xx, showing: errors.length };
  }, [errors]);

  const apm = useMemo(() => {
    if (snapshots.length === 0) {
      return { avgRpm: 0, maxP95: 0, avgLatency: overview?.avg_latency_ms ?? 0, latest: null as SystemMetricsSnapshot | null };
    }
    let sumRpm = 0;
    let sumLat = 0;
    let maxP95 = 0;
    for (const s of snapshots) {
      sumRpm += s.rpm;
      sumLat += s.avg_latency_ms;
      if (s.p95_latency_ms > maxP95) maxP95 = s.p95_latency_ms;
    }
    const n = snapshots.length;
    return {
      avgRpm: Math.round(sumRpm / n),
      maxP95,
      avgLatency: Math.round(sumLat / n),
      latest: snapshots[n - 1] ?? null,
    };
  }, [snapshots, overview?.avg_latency_ms]);

  const maxBarRpm = useMemo(() => {
    let m = 50;
    for (const s of snapshots) if (s.rpm > m) m = s.rpm;
    return m;
  }, [snapshots]);

  const copy = (id: string, text: string) => {
    void navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1600);
  };

  const tabs: { id: Tab; label: string; badge?: number }[] = [
    { id: 'saude', label: 'Saúde' },
    { id: 'apm', label: 'Desempenho' },
    {
      id: 'erros',
      label: 'Erros',
      badge: overview?.open_errors_count,
    },
  ];

  return (
    <div className="space-y-6 pb-16 max-w-6xl mx-auto">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between border-b border-border pb-5">
        <div>
          <div className="mb-2 inline-flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-[11px] font-black text-emerald-700">
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-60" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-500" />
            </span>
            Observabilidade ao vivo
          </div>
          <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">Telemetria</h1>
          <p className="mt-1 text-xs sm:text-sm text-ink-muted max-w-md">
            Saúde do stack, desempenho e exceções — separados por aba, sem scroll infinito.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex items-center gap-0.5 rounded-2xl border border-border bg-paper p-1 shadow-xs text-[11px] font-bold">
            <span className="px-2 text-ink-muted hidden sm:inline">Autoatualização</span>
            {([
              { label: '5s', value: 5_000 as const },
              { label: '10s', value: 10_000 as const },
              { label: '30s', value: 30_000 as const },
              { label: 'Off', value: false as const },
            ]).map((o) => (
              <button
                key={o.label}
                type="button"
                onClick={() => setAutoMs(o.value)}
                className={clsx(
                  'rounded-xl px-2.5 py-1.5 transition',
                  autoMs === o.value
                    ? 'bg-bitcoin text-white shadow-sm shadow-bitcoin/25'
                    : 'text-ink-muted hover:text-ink hover:bg-surface',
                )}
              >
                {o.label}
              </button>
            ))}
          </div>
          <button
            type="button"
            onClick={() => {
              void overviewQ.refetch();
              void errorsQ.refetch();
              void metricsQ.refetch();
              void statsQ.refetch();
            }}
            className="inline-flex items-center gap-2 rounded-2xl border border-border bg-paper px-3.5 py-2 text-xs font-bold text-ink shadow-xs hover:bg-surface transition"
          >
            <i className={`bi bi-arrow-repeat ${overviewQ.isFetching ? 'animate-spin text-bitcoin' : ''}`} />
            Atualizar
          </button>
        </div>
      </header>

      <nav className="flex gap-1 rounded-2xl border border-border bg-surface/80 p-1 shadow-inner">
        {tabs.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            className={clsx(
              'flex-1 sm:flex-none rounded-xl px-4 py-2.5 text-xs font-black transition inline-flex items-center justify-center gap-2',
              tab === t.id
                ? 'bg-paper text-ink shadow-xs border border-border/80'
                : 'text-ink-muted hover:text-ink',
            )}
          >
            <i
              className={clsx(
                'bi text-sm',
                t.id === 'saude' && 'bi-heart-pulse',
                t.id === 'apm' && 'bi-graph-up-arrow',
                t.id === 'erros' && 'bi-bug',
              )}
            />
            {t.label}
            {typeof t.badge === 'number' && t.badge > 0 && (
              <span className="rounded-full bg-rose-500 text-white px-1.5 py-0.5 text-[10px] font-black min-w-[1.25rem] text-center">
                {t.badge}
              </span>
            )}
          </button>
        ))}
      </nav>

      {tab === 'saude' && (
        <div className="space-y-5">
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
            <MetricCard
              icon="bi-activity"
              iconTone="emerald"
              label="Saúde geral"
              value={`${(overview?.system_health_pct ?? 100).toFixed(1)}%`}
              hint={`Latência média ${overview?.avg_latency_ms ?? '—'} ms`}
              tone="good"
              badge="Operacional"
            />
            <MetricCard
              icon="bi-exclamation-triangle"
              iconTone={(overview?.open_errors_count ?? 0) > 0 ? 'amber' : 'emerald'}
              label="Erros abertos"
              value={overview?.open_errors_count ?? 0}
              hint={
                (overview?.critical_errors_count ?? 0) > 0
                  ? `${overview?.critical_errors_count} críticos`
                  : 'Nenhum crítico'
              }
              tone={(overview?.open_errors_count ?? 0) > 0 ? 'warn' : 'good'}
            />
            <MetricCard
              icon="bi-memory"
              iconTone="sky"
              label="Memória"
              value={server ? `${server.memory_pct}%` : '—'}
              hint={server ? `${server.memory_used_mb} / usada` : 'Consultando…'}
              progress={server?.memory_pct}
            />
            <MetricCard
              icon="bi-database"
              iconTone="bitcoin"
              label="Pool Postgres"
              value={`${server?.db_connections_active ?? overview?.active_db_connections ?? 0}/${server?.db_connections_max ?? 50}`}
              hint={`Carga 1m · ${(server?.cpu_load_1m ?? 0).toFixed(2)}`}
              progress={
                server
                  ? (server.db_connections_active / Math.max(server.db_connections_max, 1)) * 100
                  : undefined
              }
            />
          </div>

          <section className="rounded-3xl border border-border bg-paper p-5 shadow-xs">
            <div className="flex items-center justify-between gap-2 mb-4">
              <div>
                <h2 className="text-sm font-black text-ink">Serviços</h2>
                <p className="text-[11px] text-ink-muted mt-0.5">Status do ecossistema SatsPay</p>
              </div>
              <span className="rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2.5 py-1 text-[10px] font-black uppercase tracking-wide text-emerald-700">
                {server?.status ?? 'healthy'}
              </span>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-2.5">
              {[
                { name: 'API Server', detail: 'Rust · :4000', icon: 'bi-cpu' },
                { name: 'Worker', detail: 'Varredura RPC', icon: 'bi-gear-wide-connected' },
                { name: 'PostgreSQL', detail: 'Pool ativo', icon: 'bi-database-check' },
                { name: 'Kafka', detail: 'Event bus', icon: 'bi-hdd-network' },
                { name: 'Client', detail: 'Nginx · :4500', icon: 'bi-window-desktop' },
              ].map((s) => (
                <div
                  key={s.name}
                  className="rounded-2xl border border-border bg-surface/40 p-3.5 flex items-center gap-3 hover:border-emerald-500/30 transition"
                >
                  <div className="h-10 w-10 rounded-xl bg-emerald-500/10 text-emerald-600 flex items-center justify-center text-base shrink-0">
                    <i className={`bi ${s.icon}`} />
                  </div>
                  <div className="min-w-0">
                    <p className="text-xs font-black text-ink truncate">{s.name}</p>
                    <p className="text-[10px] text-ink-muted font-medium flex items-center gap-1 mt-0.5">
                      <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
                      {s.detail}
                    </p>
                  </div>
                </div>
              ))}
            </div>
            <div className="mt-4 pt-4 border-t border-border flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-ink-muted font-medium">
              <span>
                <b className="text-ink font-mono">{overview?.total_users ?? 0}</b> usuários
              </span>
              <span>
                <b className="text-ink font-mono">{overview?.total_wallets ?? 0}</b> carteiras
              </span>
              <span>
                <b className="text-ink font-mono">{overview?.total_merchants ?? 0}</b> merchants
              </span>
            </div>
          </section>

          {(overview?.open_errors_count ?? 0) > 0 && (
            <button
              type="button"
              onClick={() => setTab('erros')}
              className="group w-full rounded-2xl border border-amber-500/35 bg-gradient-to-r from-amber-500/10 to-rose-500/5 px-5 py-4 text-left transition hover:border-amber-500/50"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-3">
                  <div className="h-10 w-10 rounded-xl bg-amber-500/15 text-amber-700 flex items-center justify-center">
                    <i className="bi bi-bell-fill" />
                  </div>
                  <div>
                    <p className="text-sm font-black text-amber-900 dark:text-amber-200">
                      {overview?.open_errors_count} erros abertos
                    </p>
                    <p className="text-[11px] text-amber-800/80 dark:text-amber-200/70">
                      Abrir aba Erros para triagem
                    </p>
                  </div>
                </div>
                <i className="bi bi-arrow-right text-amber-700 group-hover:translate-x-0.5 transition" />
              </div>
            </button>
          )}
        </div>
      )}

      {tab === 'apm' && (
        <div className="space-y-5">
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
            <div>
              <h2 className="text-base font-black text-ink">Desempenho</h2>
              <p className="text-[11px] text-ink-muted">Vazão, latência e amostras do worker</p>
            </div>
            <div className="inline-flex rounded-2xl border border-border bg-paper p-1 shadow-xs text-[11px] font-bold">
              {([6, 12, 24, 48, 168] as const).map((h) => (
                <button
                  key={h}
                  type="button"
                  onClick={() => setMetricsHours(h)}
                  className={clsx(
                    'rounded-xl px-2.5 py-1.5 transition',
                    metricsHours === h ? 'bg-ink text-paper' : 'text-ink-muted hover:text-ink',
                  )}
                >
                  {h === 168 ? '7d' : `${h}h`}
                </button>
              ))}
            </div>
          </div>

          <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
            <MetricCard icon="bi-speedometer2" iconTone="bitcoin" label="Req/min" value={apm.latest?.rpm ?? apm.avgRpm} hint={`média ${apm.avgRpm} req/min`} />
            <MetricCard icon="bi-lightning-charge" iconTone="amber" label="P95 pico" value={`${apm.maxP95} ms`} hint="pior latência na janela" />
            <MetricCard icon="bi-stopwatch" iconTone="sky" label="Latência média" value={`${apm.avgLatency} ms`} hint="resposta API" />
            <MetricCard icon="bi-collection" iconTone="emerald" label="Amostras" value={snapshots.length} hint="~60s via worker" />
          </div>

          <section className="rounded-3xl border border-border bg-paper p-5 shadow-xs">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-2">
                <div className="h-8 w-8 rounded-xl bg-bitcoin/10 text-bitcoin flex items-center justify-center text-sm">
                  <i className="bi bi-bar-chart-fill" />
                </div>
                <div>
                  <h3 className="text-sm font-black text-ink">Vazão</h3>
                  <p className="text-[10px] text-ink-muted font-mono">
                    {apm.latest
                      ? `última · ${new Date(apm.latest.captured_at).toLocaleTimeString('pt-BR')}`
                      : 'aguardando coleta'}
                  </p>
                </div>
              </div>
            </div>
            {snapshots.length === 0 ? (
              <div className="py-12 text-center rounded-2xl border border-dashed border-border bg-surface/30">
                <i className="bi bi-hourglass-split text-2xl text-ink-muted/50 block mb-2" />
                <p className="text-xs text-ink-muted font-medium">Worker ainda não enviou snapshots.</p>
              </div>
            ) : (
              <div className="flex items-end gap-1 h-28 rounded-2xl bg-surface/40 border border-border px-3 pt-4 pb-2 overflow-x-auto">
                {snapshots.map((s, idx) => {
                  const h = Math.max(14, Math.round((s.rpm / maxBarRpm) * 100));
                  const bad = Number(s.error_rate_pct) > 0;
                  return (
                    <div
                      key={s.id || idx}
                      title={`${new Date(s.captured_at).toLocaleTimeString()} · ${s.rpm} rpm · ${s.avg_latency_ms}ms`}
                      className={clsx(
                        'flex-1 min-w-[7px] max-w-[20px] rounded-t-md transition-opacity hover:opacity-80',
                        bad
                          ? 'bg-rose-500'
                          : idx === snapshots.length - 1
                            ? 'bg-bitcoin shadow-sm shadow-bitcoin/30'
                            : 'bg-bitcoin/45',
                      )}
                      style={{ height: `${h}%` }}
                    />
                  );
                })}
              </div>
            )}
          </section>
        </div>
      )}

      {tab === 'erros' && (
        <div className="space-y-4">
          <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs space-y-3">
            <div className="flex flex-wrap items-end justify-between gap-3">
              <div>
                <h2 className="text-lg font-black text-ink tracking-tight">Admin Erros</h2>
                <p className="text-[11px] text-ink-muted mt-0.5">
                  Grupos por fingerprint — severidade, impacto, categoria e ciclo. Segredos redigidos.
                </p>
              </div>
              <span className="inline-flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-[11px] font-black text-emerald-700">
                <span className="relative flex h-2 w-2">
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-60" />
                  <span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-500" />
                </span>
                Ao vivo
              </span>
            </div>

            <div className="grid grid-cols-3 gap-2 sm:max-w-md">
              <StatPill label="Grupos" value={errorStats.groups} />
              <StatPill label="Grupos 5xx" value={errorStats.groups5xx} tone={errorStats.groups5xx > 0 ? 'warn' : 'good'} />
              <StatPill label="Exibindo" value={errorStats.showing} />
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <div className="inline-flex rounded-xl border border-border bg-surface p-0.5 text-[11px] font-bold">
                {(
                  [
                    { id: 'recent' as const, label: 'Recentes' },
                    { id: 'frequent' as const, label: 'Mais frequentes' },
                  ] as const
                ).map((o) => (
                  <button
                    key={o.id}
                    type="button"
                    onClick={() => setErrorSort(o.id)}
                    className={clsx(
                      'rounded-lg px-2.5 py-1.5 whitespace-nowrap transition',
                      errorSort === o.id ? 'bg-paper text-ink shadow-xs' : 'text-ink-muted hover:text-ink',
                    )}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
              <div className="inline-flex rounded-xl border border-border bg-surface p-0.5 text-[11px] font-bold overflow-x-auto max-w-full">
                {(['ALL', 'OPEN', 'INVESTIGATING', 'RESOLVED', 'IGNORED'] as const).map((s) => (
                  <button
                    key={s}
                    type="button"
                    onClick={() => setStatusFilter(s)}
                    className={clsx(
                      'rounded-lg px-2.5 py-1.5 whitespace-nowrap transition',
                      statusFilter === s ? 'bg-paper text-ink shadow-xs' : 'text-ink-muted hover:text-ink',
                    )}
                  >
                    {s === 'ALL'
                      ? 'Todos'
                      : s === 'OPEN'
                        ? 'Abertos'
                        : s === 'INVESTIGATING'
                          ? 'Invest.'
                          : s === 'RESOLVED'
                            ? 'Resolvidos'
                            : 'Ignorados'}
                  </button>
                ))}
              </div>
              <select
                value={serviceFilter}
                onChange={(e) => setServiceFilter(e.target.value)}
                className="rounded-xl border border-border bg-surface px-2.5 py-1.5 text-xs font-bold"
              >
                <option value="ALL">Origem</option>
                <option value="api-server">API</option>
                <option value="worker">Worker</option>
                <option value="client-frontend">Cliente</option>
              </select>
              <select
                value={levelFilter}
                onChange={(e) => setLevelFilter(e.target.value)}
                className="rounded-xl border border-border bg-surface px-2.5 py-1.5 text-xs font-bold"
              >
                <option value="ALL">Sev</option>
                <option value="CRITICAL">Crítico</option>
                <option value="ERROR">Erro</option>
                <option value="WARN">Aviso</option>
              </select>
              <div className="relative flex-1 min-w-[160px]">
                <i className="bi bi-search absolute left-3 top-1/2 -translate-y-1/2 text-ink-muted text-xs" />
                <input
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Código, mensagem, path…"
                  className="w-full rounded-xl border border-border bg-surface pl-8 pr-3 py-1.5 text-xs"
                />
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <label className="inline-flex items-center gap-2 rounded-xl border border-border bg-surface px-3 py-1.5 text-[11px] font-bold text-ink-muted cursor-pointer">
                <input
                  type="checkbox"
                  checked={hideNoise}
                  onChange={(e) => setHideNoise(e.target.checked)}
                  className="rounded border-border"
                />
                Esconder ruído (auth 401 / CDN / img)
              </label>
              <button
                type="button"
                disabled={resolveAllMutation.isPending || (overview?.open_errors_count ?? 0) === 0}
                onClick={() => {
                  if (window.confirm('Resolver todos os erros abertos?')) resolveAllMutation.mutate();
                }}
                className="rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white px-3 py-1.5 text-xs font-black disabled:opacity-40 transition"
              >
                Resolver abertos
              </button>
              <button
                type="button"
                disabled={clearResolvedMutation.isPending}
                onClick={() => clearResolvedMutation.mutate()}
                className="rounded-xl border border-border bg-paper px-3 py-1.5 text-xs font-bold text-ink-muted hover:text-ink transition"
              >
                Limpar resolvidos
              </button>
            </div>
          </div>

          <div className="rounded-3xl border border-border bg-paper overflow-hidden shadow-xs">
            <div className="px-4 py-3 border-b border-border bg-surface/50 flex items-center justify-between gap-2">
              <div className="flex items-center gap-2 min-w-0">
                <i className="bi bi-bug text-rose-500" />
                <h3 className="text-sm font-black text-ink">Erros</h3>
              </div>
              <span className="rounded-full bg-surface border border-border px-2.5 py-0.5 font-mono text-[11px] font-bold text-ink-muted">
                {errors.length}
              </span>
            </div>

            {errorsQ.isLoading ? (
              <p className="py-14 text-center text-xs text-ink-muted animate-pulse">Carregando telemetria…</p>
            ) : errors.length === 0 ? (
              <div className="py-14 text-center space-y-2">
                <div className="mx-auto h-12 w-12 rounded-2xl bg-emerald-500/10 text-emerald-600 flex items-center justify-center text-2xl">
                  <i className="bi bi-check2-circle" />
                </div>
                <p className="text-sm font-black text-ink">Nada por aqui</p>
                <p className="text-xs text-ink-muted">Nenhum grupo com esses filtros.</p>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-[11px] min-w-[960px]">
                  <thead>
                    <tr className="border-b border-border bg-surface/40 text-[10px] font-black uppercase tracking-wide text-ink-muted">
                      <th className="px-3 py-2.5">Sev</th>
                      <th className="px-3 py-2.5">Impacto</th>
                      <th className="px-3 py-2.5">Cat</th>
                      <th className="px-3 py-2.5">Ciclo</th>
                      <th className="px-3 py-2.5">Código</th>
                      <th className="px-3 py-2.5">Origem</th>
                      <th className="px-3 py-2.5">HTTP</th>
                      <th className="px-3 py-2.5">Qtd</th>
                      <th className="px-3 py-2.5">Amostra</th>
                      <th className="px-3 py-2.5">Path</th>
                      <th className="px-3 py-2.5">error_id</th>
                      <th className="px-3 py-2.5">Último</th>
                      <th className="px-3 py-2.5" />
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {errors.map((err) => {
                      const c = classifyError(err);
                      const open = expandedId === err.id;
                      return (
                        <ErrorGroupRow
                          key={err.id}
                          err={err}
                          classified={c}
                          open={open}
                          copiedId={copiedId}
                          onToggle={() => setExpandedId(open ? null : err.id)}
                          onCopy={copy}
                          onResolve={() => resolveMutation.mutate(err.id)}
                          onIgnore={() => ignoreMutation.mutate(err.id)}
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

function StatPill({
  label,
  value,
  tone = 'default',
}: {
  label: string;
  value: number;
  tone?: 'default' | 'good' | 'warn';
}) {
  const v =
    tone === 'warn' ? 'text-amber-700' : tone === 'good' ? 'text-emerald-700' : 'text-ink';
  return (
    <div className="rounded-xl border border-border bg-surface/50 px-3 py-2">
      <p className="text-[10px] font-bold uppercase tracking-wide text-ink-muted">{label}</p>
      <p className={clsx('text-lg font-black tabular-nums', v)}>{value}</p>
    </div>
  );
}

function badgeTone(kind: 'sev' | 'impact' | 'cat' | 'ciclo', value: string): string {
  if (kind === 'sev') {
    if (value === 'FATAL' || value === 'ERROR') return 'border-rose-500/40 bg-rose-500/15 text-rose-700';
    if (value === 'WARNING') return 'border-amber-500/40 bg-amber-500/15 text-amber-800';
    return 'border-border bg-surface text-ink-muted';
  }
  if (kind === 'impact') {
    if (value === 'CRITICAL' || value === 'HIGH') return 'border-rose-500/40 bg-rose-500/10 text-rose-700';
    if (value === 'MEDIUM') return 'border-amber-500/35 bg-amber-500/10 text-amber-800';
    return 'border-border bg-surface text-ink-muted';
  }
  if (kind === 'ciclo') {
    if (value === 'NEW') return 'border-sky-500/40 bg-sky-500/10 text-sky-800';
    if (value === 'RESOLVED') return 'border-emerald-500/40 bg-emerald-500/10 text-emerald-800';
    if (value === 'IGNORED') return 'border-border bg-surface text-ink-muted';
    return 'border-border bg-surface text-ink';
  }
  return 'border-border bg-surface text-ink';
}

function ErrorGroupRow({
  err,
  classified: c,
  open,
  copiedId,
  onToggle,
  onCopy,
  onResolve,
  onIgnore,
}: {
  err: SystemErrorLog;
  classified: ClassifiedError;
  open: boolean;
  copiedId: string | null;
  onToggle: () => void;
  onCopy: (id: string, text: string) => void;
  onResolve: () => void;
  onIgnore: () => void;
}) {
  return (
    <>
      <tr className="hover:bg-surface/40 transition-colors align-top">
        <td className="px-3 py-2.5">
          <span className={clsx('rounded-md border px-1.5 py-0.5 font-mono font-black', badgeTone('sev', c.severity))}>
            {c.severity}
          </span>
        </td>
        <td className="px-3 py-2.5">
          <span className={clsx('rounded-md border px-1.5 py-0.5 font-mono font-black', badgeTone('impact', c.impact))}>
            {c.impact}
          </span>
        </td>
        <td className="px-3 py-2.5 font-mono font-bold text-ink">{c.category}</td>
        <td className="px-3 py-2.5">
          <span className={clsx('rounded-md border px-1.5 py-0.5 font-mono font-black', badgeTone('ciclo', c.lifecycle))}>
            {c.lifecycle}
          </span>
        </td>
        <td className="px-3 py-2.5 font-mono font-bold text-ink whitespace-nowrap">{c.code}</td>
        <td className="px-3 py-2.5 font-mono text-ink-muted">{c.origin}</td>
        <td className="px-3 py-2.5 font-mono tabular-nums">{err.status_code ?? '—'}</td>
        <td className="px-3 py-2.5 font-mono font-bold tabular-nums">{err.occurrences_count}</td>
        <td className="px-3 py-2.5 font-mono text-ink max-w-[220px] truncate" title={c.sample}>
          {c.sample}
        </td>
        <td className="px-3 py-2.5 font-mono text-ink-muted max-w-[140px] truncate" title={c.path}>
          {c.path}
        </td>
        <td className="px-3 py-2.5 font-mono text-ink-muted whitespace-nowrap">{c.errorIdShort}</td>
        <td className="px-3 py-2.5 text-ink-muted whitespace-nowrap">{formatAdminDate(err.last_seen_at)}</td>
        <td className="px-3 py-2.5">
          <div className="flex items-center gap-1 justify-end">
            <button
              type="button"
              onClick={onToggle}
              className="rounded-lg border border-border bg-surface px-2 py-1 text-[10px] font-bold hover:bg-paper"
            >
              {open ? '−' : '+'}
            </button>
            {err.status !== 'RESOLVED' && (
              <button
                type="button"
                onClick={onResolve}
                className="rounded-lg bg-emerald-600 text-white px-2 py-1 text-[10px] font-black"
              >
                OK
              </button>
            )}
            {err.status !== 'IGNORED' && (
              <button
                type="button"
                onClick={onIgnore}
                className="rounded-lg border border-border px-2 py-1 text-[10px] font-bold text-ink-muted"
              >
                Ign
              </button>
            )}
          </div>
        </td>
      </tr>
      {open && (
        <tr className="bg-surface/30">
          <td colSpan={13} className="px-4 py-3">
            <div className="rounded-2xl border border-border bg-paper p-4 space-y-3 text-[11px]">
              <p className="font-mono text-ink break-words">{redactSecrets(err.message)}</p>
              {err.stack_trace && (
                <div>
                  <div className="flex justify-between mb-1.5">
                    <span className="font-black uppercase tracking-wider text-ink-muted">Stack</span>
                    <button
                      type="button"
                      className="font-bold text-bitcoin hover:underline"
                      onClick={() => onCopy(err.id, redactSecrets(err.stack_trace))}
                    >
                      {copiedId === err.id ? 'Copiado' : 'Copiar'}
                    </button>
                  </div>
                  <pre className="max-h-44 overflow-auto rounded-xl bg-surface border border-border p-3 font-mono text-[10px] whitespace-pre-wrap">
                    {redactSecrets(err.stack_trace)}
                  </pre>
                </div>
              )}
              <p className="font-mono text-ink-muted">fp · {err.fingerprint}</p>
              {!!err.request_payload?.breadcrumbs?.length && (
                <div>
                  <p className="font-black uppercase tracking-wider text-ink-muted mb-1.5">Breadcrumbs</p>
                  <ul className="space-y-1 font-mono text-[10px] text-ink-muted">
                    {err.request_payload.breadcrumbs.slice(-8).map((b, i) => (
                      <li key={`${b.timestamp}-${i}`} className="rounded-lg bg-surface border border-border px-2 py-1">
                        <span className="text-ink font-bold">[{b.category}]</span> {redactSecrets(b.message)}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

function MetricCard({
  label,
  value,
  hint,
  icon,
  iconTone = 'bitcoin',
  tone = 'default',
  badge,
  progress,
}: {
  label: string;
  value: string | number;
  hint?: string;
  icon: string;
  iconTone?: 'bitcoin' | 'emerald' | 'amber' | 'sky';
  tone?: 'default' | 'good' | 'warn';
  badge?: string;
  progress?: number;
}) {
  const valueCls =
    tone === 'good' ? 'text-emerald-600' : tone === 'warn' ? 'text-amber-600' : 'text-ink';
  const iconWrap =
    iconTone === 'emerald'
      ? 'bg-emerald-500/10 text-emerald-600'
      : iconTone === 'amber'
        ? 'bg-amber-500/10 text-amber-600'
        : iconTone === 'sky'
          ? 'bg-sky-500/10 text-sky-600'
          : 'bg-bitcoin/10 text-bitcoin';
  const barCls =
    (progress ?? 0) > 85 ? 'bg-rose-500' : (progress ?? 0) > 65 ? 'bg-amber-500' : 'bg-bitcoin';

  return (
    <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs hover:border-bitcoin/25 transition">
      <div className="flex items-start justify-between gap-2 mb-3">
        <p className="text-[10px] font-black uppercase tracking-wider text-ink-muted">{label}</p>
        <div className={`h-9 w-9 rounded-2xl flex items-center justify-center text-sm shrink-0 ${iconWrap}`}>
          <i className={`bi ${icon}`} />
        </div>
      </div>
      <div className="flex items-baseline gap-2 flex-wrap">
        <p className={`text-2xl sm:text-3xl font-black font-mono tabular-nums tracking-tight ${valueCls}`}>
          {value}
        </p>
        {badge && (
          <span className="rounded-full bg-emerald-500/10 text-emerald-700 px-2 py-0.5 text-[10px] font-black">
            {badge}
          </span>
        )}
      </div>
      {hint && <p className="mt-1.5 text-[11px] text-ink-muted font-medium">{hint}</p>}
      {typeof progress === 'number' && (
        <div className="mt-3 h-1.5 rounded-full bg-border overflow-hidden">
          <div className={`h-full rounded-full transition-all ${barCls}`} style={{ width: `${Math.min(progress, 100)}%` }} />
        </div>
      )}
    </div>
  );
}

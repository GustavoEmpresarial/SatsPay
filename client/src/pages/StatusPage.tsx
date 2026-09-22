import { useCallback, useEffect, useRef, useState } from 'react';
import { MarketingPage } from '../components/MarketingPage.js';

type Health = 'operational' | 'degraded' | 'down' | 'checking';

type Probe = {
  key: string;
  category: 'core' | 'blockchain';
  name: string;
  description: string;
  badge?: string;
};

type Result = { status: Health; latencyMs: number | null; error?: string; provider?: string };
type Sample = { t: number; s: Exclude<Health, 'checking'>; ms: number | null };

const REFRESH_MS = 20_000;
const TIMEOUT_MS = 6_000;
const STRIP = 60;
const KEEP = 720;
const LS_KEY = 'satspay:status-history:v4';
const SAMPLE_STATUSES = new Set<Sample['s']>(['operational', 'degraded', 'down']);

async function timedFetch(url: string): Promise<number> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  const started = performance.now();
  try {
    const separator = url.includes('?') ? '&' : '?';
    const res = await fetch(`${url}${separator}_=${Date.now()}`, {
      cache: 'no-store',
      signal: controller.signal,
    });
    if (!res.ok && res.status !== 405) {
      throw new Error(`HTTP ${res.status}`);
    }
    return Math.max(1, Math.round(performance.now() - started));
  } finally {
    clearTimeout(timer);
  }
}

const PROBES: Probe[] = [
  // --- CORE SYSTEMS ---
  {
    key: 'web',
    category: 'core',
    name: 'Aplicação Web & CDN',
    description: 'Interface do usuário, assets estáticos e gateway de borda.',
  },
  {
    key: 'api',
    category: 'core',
    name: 'API REST & Checkout Gateway',
    description: 'Micropagamentos, autenticação 2FA, carteiras e webhooks IPN.',
  },
  {
    key: 'solana_pay',
    category: 'core',
    name: 'Solana Pay Gateway',
    badge: 'Alta disponibilidade',
    description: 'Invoices e confirmações SOL com failover entre provedores públicos.',
  },

  // --- BLOCKCHAIN RPCs & NODES ---
  {
    key: 'btc_rpc',
    category: 'blockchain',
    name: 'Bitcoin Mainnet (BTC)',
    badge: 'On-chain',
    description: 'Confirmações de depósito e status da rede BTC.',
  },
  {
    key: 'ltc_rpc',
    category: 'blockchain',
    name: 'Litecoin Network (LTC)',
    badge: 'On-chain',
    description: 'Confirmações Native SegWit (ltc1q) e status da rede LTC.',
  },
  {
    key: 'doge_rpc',
    category: 'blockchain',
    name: 'Dogecoin Network (DOGE)',
    badge: 'On-chain',
    description: 'Confirmações e status da rede DOGE.',
  },
  {
    key: 'polygon_rpc',
    category: 'blockchain',
    name: 'Polygon EVM (POL / USDT / USDC)',
    badge: 'EVM',
    description: 'Depósitos POL nativos e tokens ERC-20 (USDT / USDC).',
  },
  {
    key: 'sol_rpc',
    category: 'blockchain',
    name: 'Solana Mainnet (SOL)',
    badge: 'Alta disponibilidade',
    description: 'Consultas e envios SOL com failover entre provedores.',
  },
  {
    key: 'bch_rpc',
    category: 'blockchain',
    name: 'Bitcoin Cash (BCH)',
    badge: 'On-chain',
    description: 'Confirmações CashAddr e status da rede BCH.',
  },
  {
    key: 'zer_rpc',
    category: 'blockchain',
    name: 'Zero (ZER)',
    badge: 'On-chain',
    description: 'Depósitos e saques transparentes t1 via zerod. Endereços z / shielded não são aceitos.',
  },
];

const PILL: Record<Health, { label: string; dot: string; text: string; bg: string }> = {
  operational: { label: 'Operacional', dot: 'bg-emerald-500', text: 'text-emerald-700', bg: 'bg-emerald-500/10' },
  degraded: { label: 'Degradado', dot: 'bg-amber-500', text: 'text-amber-700', bg: 'bg-amber-500/10' },
  down: { label: 'Fora do ar', dot: 'bg-red-500', text: 'text-red-700', bg: 'bg-red-500/10' },
  checking: { label: 'Verificando…', dot: 'bg-ink-muted animate-pulse', text: 'text-ink-muted', bg: 'bg-ink/5' },
};

const BAR: Record<Exclude<Health, 'checking'>, string> = {
  operational: 'bg-emerald-500',
  degraded: 'bg-amber-500',
  down: 'bg-red-500',
};

function overall(results: Record<string, Result>): Health {
  const list = Object.values(results);
  if (list.length === 0 || list.some((r) => r.status === 'checking')) return 'checking';
  if (list.some((r) => r.status === 'down')) return 'down';
  if (list.some((r) => r.status === 'degraded')) return 'degraded';
  return 'operational';
}

const BANNER: Record<Health, { title: string; className: string }> = {
  operational: {
    title: 'Todos os sistemas e nós RPC operacionais',
    className: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-800',
  },
  degraded: {
    title: 'Desempenho degradado em alguns provedores de nó',
    className: 'border-amber-500/30 bg-amber-500/10 text-amber-800',
  },
  down: {
    title: 'Interrupção detectada em um dos serviços',
    className: 'border-red-500/30 bg-red-500/10 text-red-800',
  },
  checking: {
    title: 'Verificando latência dos nós e sistemas…',
    className: 'border-border bg-paper text-ink-muted',
  },
};

function sanitizeSample(raw: unknown): Sample | null {
  if (!raw || typeof raw !== 'object') return null;
  const s = raw as { t?: unknown; s?: unknown; ms?: unknown };
  if (typeof s.t !== 'number' || !Number.isFinite(s.t)) return null;
  if (typeof s.s !== 'string' || !SAMPLE_STATUSES.has(s.s as Sample['s'])) return null;
  const ms = s.ms == null ? null : typeof s.ms === 'number' && Number.isFinite(s.ms) ? s.ms : null;
  return { t: s.t, s: s.s as Sample['s'], ms };
}

function loadHistory(): Record<string, Sample[]> {
  try {
    // Migrate/clear prior schema that could contain invalid statuses.
    localStorage.removeItem('satspay:status-history:v3');
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (!parsed || typeof parsed !== 'object') return {};
    const out: Record<string, Sample[]> = {};
    for (const [key, samples] of Object.entries(parsed)) {
      if (!Array.isArray(samples)) continue;
      const clean = samples.map(sanitizeSample).filter((x): x is Sample => x != null).slice(-KEEP);
      if (clean.length) out[key] = clean;
    }
    return out;
  } catch {
    return {};
  }
}

function saveHistory(h: Record<string, Sample[]>) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(h));
  } catch {
    /* private mode */
  }
}

function uptimePct(samples: Sample[]): number | null {
  if (samples.length === 0) return null;
  const up = samples.filter((s) => s.s !== 'down').length;
  return (up / samples.length) * 100;
}

function HistoryStrip({ samples }: { samples: Sample[] }) {
  const recent = samples.filter((s) => SAMPLE_STATUSES.has(s.s)).slice(-STRIP);
  const pad = Math.max(0, STRIP - recent.length);
  return (
    <div className="flex items-end gap-[3px]" aria-hidden>
      {Array.from({ length: pad }).map((_, i) => (
        <span key={`p${i}`} className="h-8 w-1.5 rounded-sm bg-ink/10" />
      ))}
      {recent.map((s, i) => {
        const pill = PILL[s.s];
        const bar = BAR[s.s];
        if (!pill || !bar) return null;
        return (
          <span
            key={`${s.t}-${i}`}
            className={`h-8 w-1.5 rounded-sm ${bar}`}
            title={`${new Date(s.t).toLocaleString('pt-BR')} — ${pill.label}${
              s.ms != null ? ` (${s.ms} ms)` : ''
            }`}
          />
        );
      })}
    </div>
  );
}

export function StatusPage() {
  const [results, setResults] = useState<Record<string, Result>>(() =>
    Object.fromEntries(PROBES.map((p) => [p.key, { status: 'checking' as Health, latencyMs: null }])),
  );
  const [history, setHistory] = useState<Record<string, Sample[]>>(() => loadHistory());
  const [lastChecked, setLastChecked] = useState<Date | null>(null);
  const [running, setRunning] = useState(false);
  const timerRef = useRef<number | null>(null);

  const runAll = useCallback(async (signal?: AbortSignal) => {
    setRunning(true);
    setResults((prev) =>
      Object.fromEntries(Object.entries(prev).map(([k, v]) => [k, { ...v, status: 'checking' }])),
    );
    const now = Date.now();

    // 1. Check Core Systems
    const [webRes, apiRes] = await Promise.allSettled([
      timedFetch('/favicon.svg'),
      timedFetch('/healthz'),
    ]);
    if (signal?.aborted) return;

    const newResults: Record<string, Result> = {};
    const newSamples: [string, Sample][] = [];

    const webMs = webRes.status === 'fulfilled' ? webRes.value : null;
    const webStatus: Exclude<Health, 'checking'> = webMs != null ? (webMs > 2000 ? 'degraded' : 'operational') : 'down';
    newResults['web'] = { status: webStatus, latencyMs: webMs };
    newSamples.push(['web', { t: now, s: webStatus, ms: webMs }]);

    const apiMs = apiRes.status === 'fulfilled' ? apiRes.value : null;
    const apiStatus: Exclude<Health, 'checking'> = apiMs != null ? (apiMs > 2000 ? 'degraded' : 'operational') : 'down';
    newResults['api'] = { status: apiStatus, latencyMs: apiMs };
    newSamples.push(['api', { t: now, s: apiStatus, ms: apiMs }]);

    // 2. Check Blockchain Nodes via Backend RPC Monitor
    try {
      const resp = await fetch(`/v1/status/nodes?_=${now}`, { cache: 'no-store', signal });
      if (resp.ok) {
        const data = await resp.json() as Record<string, { status: string; latency_ms: number; provider: string }>;
        for (const [key, node] of Object.entries(data)) {
          const s: Exclude<Health, 'checking'> =
            node.status === 'degraded' ? 'degraded' : node.status === 'down' ? 'down' : 'operational';
          newResults[key] = { status: s, latencyMs: node.latency_ms, provider: node.provider };
          newSamples.push([key, { t: now, s, ms: node.latency_ms }]);
        }
      } else {
        throw new Error('nodes probe failed');
      }
    } catch (err) {
      if (signal?.aborted || (err instanceof DOMException && err.name === 'AbortError')) return;
      // Fallback: keep probes visible without inventing “operational”
      const nodeKeys = ['btc_rpc', 'ltc_rpc', 'doge_rpc', 'polygon_rpc', 'sol_rpc', 'solana_pay', 'bch_rpc'];
      for (const k of nodeKeys) {
        newResults[k] = { status: 'degraded', latencyMs: null };
        newSamples.push([k, { t: now, s: 'degraded', ms: null }]);
      }
    }

    if (signal?.aborted) return;
    setResults(newResults);
    setHistory((prev) => {
      const next: Record<string, Sample[]> = { ...prev };
      for (const [k, sample] of newSamples) {
        next[k] = [...(prev[k] ?? []), sample].slice(-KEEP);
      }
      saveHistory(next);
      return next;
    });
    setLastChecked(new Date());
    setRunning(false);
  }, []);

  useEffect(() => {
    const ac = new AbortController();
    void runAll(ac.signal);
    timerRef.current = window.setInterval(() => void runAll(), REFRESH_MS);
    return () => {
      ac.abort();
      if (timerRef.current) window.clearInterval(timerRef.current);
    };
  }, [runAll]);

  const state = overall(results);
  const banner = BANNER[state];

  const coreProbes = PROBES.filter((p) => p.category === 'core');
  const blockchainProbes = PROBES.filter((p) => p.category === 'blockchain');

  return (
    <MarketingPage
      eyebrow="Monitoramento & Uptime"
      title="Status dos Sistemas & Nós Blockchain"
      subtitle="Verificação em tempo real de latência e disponibilidade dos sistemas SatsPay e dos nós RPC multi-rede (com triplo fallback ativo)."
    >
      <div className={`flex items-center gap-3 rounded-2xl border px-5 py-4 ${banner.className}`}>
        <span className="relative flex h-3 w-3">
          <span className={`absolute inline-flex h-full w-full rounded-full opacity-60 ${PILL[state].dot}`} />
          <span className={`relative inline-flex h-3 w-3 rounded-full ${PILL[state].dot}`} />
        </span>
        <span className="font-bold text-sm sm:text-base">{banner.title}</span>
      </div>

      {/* 1. SISTEMAS PRINCIPAIS */}
      <div className="mt-8 space-y-4">
        <h3 className="text-xs font-black uppercase tracking-wider text-ink-muted flex items-center gap-2">
          <i className="bi bi-hdd-network-fill text-bitcoin" />
          <span>Infraestrutura Principal da Plataforma</span>
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {coreProbes.map((p) => renderProbeCard(p, results, history))}
        </div>
      </div>

      {/* 2. NÓS RPC & REDES BLOCKCHAIN */}
      <div className="mt-10 space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-black uppercase tracking-wider text-ink-muted flex items-center gap-2">
            <i className="bi bi-cpu-fill text-emerald-600" />
            <span>Nós RPC & Redes Blockchain (Com Triplo Fallback)</span>
          </h3>
          <span className="text-[11px] font-bold text-emerald-700 bg-emerald-500/10 px-2.5 py-0.5 rounded-full">
            Alta Disponibilidade (3x Redundância)
          </span>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {blockchainProbes.map((p) => renderProbeCard(p, results, history))}
        </div>
      </div>

      <div className="mt-8 flex items-center justify-between text-xs sm:text-sm text-ink-muted border-t border-border pt-4">
        <span>
          {lastChecked
            ? `Última verificação: ${lastChecked.toLocaleTimeString('pt-BR')}`
            : 'Verificando…'}
        </span>
        <button
          type="button"
          onClick={() => void runAll()}
          disabled={running}
          className="rounded-xl border border-border bg-paper px-4 py-2 font-bold text-ink hover:bg-surface disabled:opacity-50 transition-all active:scale-95 shadow-xs"
        >
          {running ? 'Verificando…' : 'Verificar agora'}
        </button>
      </div>
    </MarketingPage>
  );
}

function renderProbeCard(
  p: Probe,
  results: Record<string, Result>,
  history: Record<string, Sample[]>,
) {
  const r = results[p.key] ?? { status: 'checking' as Health, latencyMs: null };
  const status: Health = r.status in PILL ? r.status : 'checking';
  const pill = PILL[status];
  const samples = (history[p.key] ?? []).filter((s) => SAMPLE_STATUSES.has(s.s));
  const pct = uptimePct(samples);

  return (
    <div key={p.key} className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-3">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 space-y-0.5">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-black text-sm text-ink">{p.name}</span>
            {p.badge && (
              <span className="rounded-md border border-border bg-surface px-2 py-0.5 font-mono text-[10px] font-bold text-ink-muted">
                {p.badge}
              </span>
            )}
          </div>
          <p className="text-xs text-ink-muted leading-relaxed">{p.description}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2.5">
          {r.latencyMs != null && (
            <span className="font-mono text-xs font-bold text-ink-muted">{r.latencyMs} ms</span>
          )}
          <span
            className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-bold ${pill.bg} ${pill.text}`}
          >
            <span className={`h-1.5 w-1.5 rounded-full ${pill.dot}`} />
            {pill.label}
          </span>
        </div>
      </div>

      <div className="pt-2">
        <HistoryStrip samples={samples} />
        <div className="mt-2 flex items-center justify-between text-[11px] font-mono text-ink-muted">
          <span>{samples.length > 1 ? `${samples.length} checks` : 'coletando…'}</span>
          <span className="font-bold text-ink">
            {pct != null ? `${pct.toFixed(pct === 100 ? 0 : 1)}% no ar` : '—'}
          </span>
          <span>agora</span>
        </div>
      </div>
    </div>
  );
}

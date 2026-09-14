import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COIN_CONFIG, COINS, formatAmount, isCoin, safeBigInt, type Coin, type WalletBalance } from '@/shared';

interface LedgerEntry {
  id: string;
  coin: Coin;
  amount: string;
  type: string;
  memo: string | null;
  createdAt: string;
}

interface PricesResponse {
  priceDecimals: number;
  prices: Record<string, string>;
}

const CREDIT_TYPES = new Set(['DEPOSIT', 'FAUCET', 'TRANSFER_IN', 'WITHDRAWAL_REVERSAL', 'ADJUSTMENT', 'STAKE_UNLOCK', 'STAKE_REWARD', 'SWAP_IN']);
const DAYS = 30;

const FALLBACK_PRICES: Record<Coin, bigint> = {
  BTC: 6_250_000_000_000n, // $62,500
  LTC: 6_850_000_000n,      // $68.50
  DOGE: 12_000_000n,        // $0.12
  BCH: 38_500_000_000n,     // $385.00
  POL: 42_000_000n,         // $0.42
  DGB: 950_000n,            // $0.0095
  SOL: 14_800_000_000n,     // $148.00
  USDT: 100_000_000n,       // $1.00
  USDC: 100_000_000n,       // $1.00
};

function bigintToNumber(v: bigint, decimals: number): number {
  const scale = 10n ** BigInt(decimals);
  const whole = Number(v / scale);
  const frac = Number(v % scale) / Number(scale);
  return whole + frac;
}

function usdValueOf(amount: bigint, coin: Coin, prices: Record<Coin, bigint>, priceDecimals: number): number {
  const price = prices[coin] || FALLBACK_PRICES[coin] || 100_000_000n;
  const cfg = COIN_CONFIG[coin];
  if (!cfg) return 0;
  const coinAmount = bigintToNumber(amount, cfg.decimals);
  const usdPrice = bigintToNumber(price, priceDecimals);
  return coinAmount * usdPrice;
}

function startOfDayISO(d: Date): string {
  const c = new Date(d);
  c.setHours(0, 0, 0, 0);
  return c.toISOString().slice(0, 10);
}

// -------- Charts --------

function Donut({
  segments,
  size = 180,
  thickness = 26,
}: {
  segments: Array<{ label: string; value: number; color: string }>;
  size?: number;
  thickness?: number;
}) {
  const total = segments.reduce((s, x) => s + x.value, 0);
  const r = (size - thickness) / 2;
  const c = 2 * Math.PI * r;
  let offset = 0;
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="mx-auto block">
      <circle cx={size / 2} cy={size / 2} r={r} strokeWidth={thickness} className="stroke-surface" fill="none" />
      {total > 0 &&
        segments.map((seg, i) => {
          const frac = seg.value / total;
          const dash = frac * c;
          const el = (
            <circle
              key={i}
              cx={size / 2}
              cy={size / 2}
              r={r}
              stroke={seg.color}
              strokeWidth={thickness}
              fill="none"
              strokeDasharray={`${dash} ${c - dash}`}
              strokeDashoffset={-offset}
              transform={`rotate(-90 ${size / 2} ${size / 2})`}
              strokeLinecap="butt"
            />
          );
          offset += dash;
          return el;
        })}
    </svg>
  );
}

function BarStack({
  days,
  size = { w: 640, h: 180 },
}: {
  days: Array<{ day: string; credit: number; debit: number }>;
  size?: { w: number; h: number };
}) {
  const max = Math.max(1, ...days.map((d) => Math.max(d.credit, d.debit)));
  const bw = size.w / days.length;
  const midY = size.h / 2;
  const barGap = 2;
  return (
    <svg viewBox={`0 0 ${size.w} ${size.h}`} className="w-full">
      <line x1={0} y1={midY} x2={size.w} y2={midY} className="stroke-border" strokeDasharray="4 4" />
      {days.map((d, i) => {
        const cH = (d.credit / max) * (midY - 8);
        const dH = (d.debit / max) * (midY - 8);
        const x = i * bw + barGap;
        const w = bw - barGap * 2;
        return (
          <g key={d.day}>
            <rect x={x} y={midY - cH} width={w} height={cH} rx={2} fill="#10b981" opacity={0.85}>
              <title>{`${d.day} · +${d.credit.toFixed(2)} USD`}</title>
            </rect>
            <rect x={x} y={midY} width={w} height={dH} rx={2} fill="#ef4444" opacity={0.85}>
              <title>{`${d.day} · -${d.debit.toFixed(2)} USD`}</title>
            </rect>
          </g>
        );
      })}
    </svg>
  );
}

function LineChart({
  points,
  size = { w: 640, h: 180 },
  color = '#f7931a',
}: {
  points: Array<{ x: string; y: number }>;
  size?: { w: number; h: number };
  color?: string;
}) {
  if (points.length === 0) return <div className="p-6 text-center text-sm text-ink-muted">—</div>;
  const max = Math.max(...points.map((p) => p.y), 1);
  const min = Math.min(...points.map((p) => p.y), 0);
  const range = max - min || 1;
  const padY = 12;
  const usableH = size.h - padY * 2;
  const step = size.w / Math.max(points.length - 1, 1);
  const coord = (i: number, y: number) => {
    const cx = i * step;
    const cy = padY + (1 - (y - min) / range) * usableH;
    return [cx, cy] as const;
  };
  const path = points.map((p, i) => {
    const [x, y] = coord(i, p.y);
    return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(' ');
  const area = `${path} L${((points.length - 1) * step).toFixed(1)},${size.h} L0,${size.h} Z`;

  return (
    <svg viewBox={`0 0 ${size.w} ${size.h}`} className="w-full">
      <defs>
        <linearGradient id="lg-area" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.35} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <path d={area} fill="url(#lg-area)" />
      <path d={path} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
      {points.map((p, i) => {
        const [x, y] = coord(i, p.y);
        return <circle key={i} cx={x} cy={y} r={2} fill={color} />;
      })}
    </svg>
  );
}

// -------- Page --------

export function AnalyticsPage() {
  const { t, i18n } = useTranslation();
  const user = useAuthStore((s) => s.user);
  const dateFmt = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? 'en', { month: 'short', day: 'numeric' });
  const usdFmt = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 2 });

  const walletsQ = useQuery({
    queryKey: ['wallets', 'personal'],
    queryFn: () => api<{ wallets: WalletBalance[] }>('/wallet'),
    enabled: Boolean(user),
  });

  const ledgerQ = useQuery({
    queryKey: ['ledger', 'analytics'],
    queryFn: () => api<{ entries: LedgerEntry[] }>('/wallet/ledger?take=200'),
    enabled: Boolean(user),
  });

  const pricesQ = useQuery({
    queryKey: ['prices'],
    queryFn: () => api<PricesResponse>('/swap/prices', { skipAuth: true }),
    staleTime: 60_000,
  });

  const prices = useMemo<Record<Coin, bigint>>(() => {
    const out = { ...FALLBACK_PRICES };
    if (pricesQ.data?.prices) {
      for (const c of COINS) {
        const p = pricesQ.data.prices[c];
        if (p) out[c] = safeBigInt(p);
      }
    }
    return out;
  }, [pricesQ.data]);

  const priceDecimals = pricesQ.data?.priceDecimals ?? 8;
  const defaultWallets: WalletBalance[] = COINS.map((c) => ({ coin: c, balance: '0', kind: 'PERSONAL' }));
  const wallets = walletsQ.data?.wallets?.length ? walletsQ.data.wallets : defaultWallets;
  const entries = useMemo(() => ledgerQ.data?.entries ?? [], [ledgerQ.data?.entries]);

  // Balance in USD per coin
  const balanceByCoin = useMemo(() => {
    return wallets.flatMap((w) => {
      if (!isCoin(w.coin)) return [];
      const bal = safeBigInt(w.balance || '0');
      const usd = usdValueOf(bal, w.coin, prices, priceDecimals);
      return [{ coin: w.coin, balance: bal, usd }];
    });
  }, [wallets, prices, priceDecimals]);

  const totalUsd = balanceByCoin.reduce((s, b) => s + b.usd, 0);

  // Daily inflow/outflow (USD, last 30 days)
  const dailySeries = useMemo(() => {
    const map = new Map<string, { credit: number; debit: number }>();
    const now = new Date();
    for (let i = DAYS - 1; i >= 0; i--) {
      const d = new Date(now);
      d.setDate(now.getDate() - i);
      map.set(startOfDayISO(d), { credit: 0, debit: 0 });
    }
    for (const e of entries) {
      const key = startOfDayISO(new Date(e.createdAt));
      if (!map.has(key)) continue;
      const amt = safeBigInt(e.amount);
      const abs = amt < 0n ? -amt : amt;
      const usd = usdValueOf(abs, e.coin, prices, priceDecimals);
      const bucket = map.get(key)!;
      if (CREDIT_TYPES.has(e.type) || amt > 0n) bucket.credit += usd;
      else bucket.debit += usd;
    }
    return Array.from(map.entries()).map(([day, v]) => ({ day, ...v }));
  }, [entries, prices, priceDecimals]);

  // Cumulative net USD over the same window
  const cumulative = useMemo(() => {
    let acc = 0;
    return dailySeries.map((d) => {
      acc += d.credit - d.debit;
      return { x: d.day, y: acc };
    });
  }, [dailySeries]);

  // Breakdown by ledger type (count + USD)
  const byType = useMemo(() => {
    const m = new Map<string, { count: number; usd: number }>();
    for (const e of entries) {
      const amt = safeBigInt(e.amount);
      const abs = amt < 0n ? -amt : amt;
      const usd = usdValueOf(abs, e.coin, prices, priceDecimals);
      const cur = m.get(e.type) ?? { count: 0, usd: 0 };
      cur.count += 1;
      cur.usd += usd;
      m.set(e.type, cur);
    }
    return Array.from(m.entries())
      .map(([type, v]) => ({ type, ...v }))
      .sort((a, b) => b.usd - a.usd);
  }, [entries, prices, priceDecimals]);

  const totalIn = dailySeries.reduce((s, d) => s + d.credit, 0);
  const totalOut = dailySeries.reduce((s, d) => s + d.debit, 0);
  const netFlow = totalIn - totalOut;

  const hasActivity = entries.length > 0 || totalUsd > 0;

  return (
    <div className="space-y-6">
      {/* Header */}
      <motion.div initial={{ opacity: 0, y: -8 }} animate={{ opacity: 1, y: 0 }}>
        <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-bitcoin-dark mb-1">
          <i className="bi bi-graph-up-arrow" />
          <span>Métricas & Performance</span>
        </div>
        <h1 className="text-2xl font-bold tracking-tight md:text-3xl">
          {t('analytics.title', { defaultValue: 'Análise de Portfólio' })}
        </h1>
        <p className="text-sm text-ink-muted mt-1">
          {t('analytics.subtitle', { defaultValue: 'Portfólio e atividade dos últimos 30 dias.' })}
        </p>
      </motion.div>

      {/* KPI CARDS */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="card p-5">
          <div className="text-[10px] font-semibold uppercase tracking-widest text-ink-muted">
            {t('analytics.totalUsd', { defaultValue: 'Valor do Portfólio' })}
          </div>
          <div className="mt-1 font-mono text-2xl font-bold text-ink">{usdFmt.format(totalUsd)}</div>
          <div className="mt-1 text-[11px] text-ink-muted">
            {balanceByCoin.filter((b) => b.balance > 0n).length} / {COINS.length} {t('analytics.coins', { defaultValue: 'moedas ativas' })}
          </div>
        </motion.div>

        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.05 }} className="card p-5">
          <div className="text-[10px] font-semibold uppercase tracking-widest text-ink-muted">
            {t('analytics.inflow30d', { defaultValue: 'Entradas (30d)' })}
          </div>
          <div className="mt-1 font-mono text-2xl font-bold text-emerald-700">+{usdFmt.format(totalIn)}</div>
          <div className="mt-1 text-[11px] text-ink-muted">{t('analytics.inflowHint', { defaultValue: 'depósitos + faucet + swap-in' })}</div>
        </motion.div>

        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.1 }} className="card p-5">
          <div className="text-[10px] font-semibold uppercase tracking-widest text-ink-muted">
            {t('analytics.outflow30d', { defaultValue: 'Saídas (30d)' })}
          </div>
          <div className="mt-1 font-mono text-2xl font-bold text-rose-700">-{usdFmt.format(totalOut)}</div>
          <div className="mt-1 text-[11px] text-ink-muted">{t('analytics.outflowHint', { defaultValue: 'saques + swap-out + stake' })}</div>
        </motion.div>

        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.15 }} className="card p-5">
          <div className="text-[10px] font-semibold uppercase tracking-widest text-ink-muted">
            {t('analytics.net30d', { defaultValue: 'Fluxo Líquido (30d)' })}
          </div>
          <div className={`mt-1 font-mono text-2xl font-bold ${netFlow >= 0 ? 'text-emerald-700' : 'text-rose-700'}`}>
            {netFlow >= 0 ? '+' : ''}
            {usdFmt.format(netFlow)}
          </div>
          <div className="mt-1 text-[11px] text-ink-muted">
            {entries.length} {t('analytics.entries', { defaultValue: 'lançamentos' })}
          </div>
        </motion.div>
      </div>

      {!hasActivity && (
        <div className="card p-8 text-center space-y-4 border border-dashed border-border">
          <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-full bg-bitcoin/10 text-2xl text-bitcoin-dark">
            <i className="bi bi-graph-up" />
          </div>
          <div>
            <h3 className="text-base font-bold text-ink">Comece a movimentar sua conta</h3>
            <p className="text-xs text-ink-muted max-w-md mx-auto mt-1 leading-relaxed">
              Você ainda não possui transações recentes nos últimos 30 dias. Reivindique moedas no Faucet ou faça um Swap para ver seus gráficos em tempo real.
            </p>
          </div>
          <div className="flex flex-wrap items-center justify-center gap-3 pt-2">
            <Link to="/faucet" className="btn-primary text-xs py-2 px-4 inline-flex items-center gap-1.5">
              <i className="bi bi-droplet-fill" /> Reivindicar Faucet
            </Link>
            <Link to="/swap" className="btn-secondary text-xs py-2 px-4 inline-flex items-center gap-1.5">
              <i className="bi bi-arrow-left-right" /> Trocar Moedas (Swap)
            </Link>
            <Link to="/stake" className="btn-secondary text-xs py-2 px-4 inline-flex items-center gap-1.5">
              <i className="bi bi-bank" /> Staking (Rendimento)
            </Link>
          </div>
        </div>
      )}

      {/* Row: donut + line */}
      <div className="grid gap-5 lg:grid-cols-12">
        <motion.section
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          className="card p-5 lg:col-span-5"
        >
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">
              {t('analytics.allocation', { defaultValue: 'Alocação por Ativo' })}
            </h2>
            <span className="text-[10px] text-ink-muted">USD</span>
          </div>
          <Donut
            segments={balanceByCoin
              .filter((b) => b.usd > 0)
              .map((b) => ({ label: b.coin, value: b.usd, color: COIN_CONFIG[b.coin]?.displayColor || '#F7931A' }))}
          />
          <ul className="mt-4 space-y-2 max-h-64 overflow-y-auto pr-1">
            {balanceByCoin.map((b) => {
              const pct = totalUsd > 0 ? (b.usd / totalUsd) * 100 : 0;
              return (
                <li key={b.coin} className="flex items-center gap-3 p-1.5 hover:bg-surface/50 rounded-lg">
                  <img src={coinLogo(b.coin)} alt={b.coin} className="h-6 w-6 rounded-full" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between text-xs">
                      <span className="font-semibold">{COIN_CONFIG[b.coin]?.symbol || b.coin}</span>
                      <span className="font-mono text-xs text-ink-muted">{pct.toFixed(1)}%</span>
                    </div>
                    <div className="flex items-center justify-between text-[11px] text-ink-muted">
                      <span className="font-mono">{formatAmount(b.balance, b.coin)} {b.coin}</span>
                      <span className="font-mono font-medium text-ink">{usdFmt.format(b.usd)}</span>
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        </motion.section>

        <motion.section
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.05 }}
          className="card p-5 lg:col-span-7"
        >
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">
              {t('analytics.netTrend', { defaultValue: 'Fluxo Líquido Acumulado' })}
            </h2>
            <span className="text-[10px] text-ink-muted">30d · USD</span>
          </div>
          <LineChart points={cumulative} />
          <div className="mt-2 flex justify-between text-[10px] text-ink-muted">
            <span>{dateFmt.format(new Date(cumulative[0]?.x ?? Date.now()))}</span>
            <span>{dateFmt.format(new Date(cumulative[cumulative.length - 1]?.x ?? Date.now()))}</span>
          </div>
        </motion.section>
      </div>

      {/* Daily bars + by-type */}
      <div className="grid gap-5 lg:grid-cols-12">
        <motion.section
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          className="card p-5 lg:col-span-7"
        >
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">
              {t('analytics.dailyFlow', { defaultValue: 'Entradas vs Saídas por Dia' })}
            </h2>
            <div className="flex items-center gap-3 text-[10px] text-ink-muted">
              <span className="inline-flex items-center gap-1">
                <span className="inline-block h-2 w-2 rounded-sm bg-emerald-500" /> {t('analytics.in', { defaultValue: 'Entradas' })}
              </span>
              <span className="inline-flex items-center gap-1">
                <span className="inline-block h-2 w-2 rounded-sm bg-rose-500" /> {t('analytics.out', { defaultValue: 'Saídas' })}
              </span>
            </div>
          </div>
          <BarStack days={dailySeries} />
          <div className="mt-2 flex justify-between text-[10px] text-ink-muted">
            <span>{dateFmt.format(new Date(dailySeries[0]?.day ?? Date.now()))}</span>
            <span>{dateFmt.format(new Date(dailySeries[dailySeries.length - 1]?.day ?? Date.now()))}</span>
          </div>
        </motion.section>

        <motion.section
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.05 }}
          className="card p-5 lg:col-span-5"
        >
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">
              {t('analytics.byType', { defaultValue: 'Por Tipo de Operação' })}
            </h2>
            <span className="text-[10px] text-ink-muted">USD</span>
          </div>
          <ul className="space-y-2">
            {byType.slice(0, 10).map((row) => {
              const maxUsd = byType[0]?.usd || 1;
              const pct = Math.max(4, (row.usd / maxUsd) * 100);
              const isCredit = CREDIT_TYPES.has(row.type);
              return (
                <li key={row.type}>
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-semibold">
                      {t(`dashboard.type.${row.type}`, { defaultValue: row.type })}
                    </span>
                    <span className="font-mono text-ink-muted">
                      {usdFmt.format(row.usd)} · {row.count}
                    </span>
                  </div>
                  <div className="mt-1 h-2 overflow-hidden rounded-full bg-surface">
                    <div
                      className={`h-full rounded-full ${isCredit ? 'bg-emerald-500' : 'bg-rose-500'}`}
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </li>
              );
            })}
            {byType.length === 0 && <li className="text-center text-xs text-ink-muted py-4">Sem operações no período</li>}
          </ul>
        </motion.section>
      </div>
    </div>
  );
}

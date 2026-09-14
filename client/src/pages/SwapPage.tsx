import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { formatApiError } from '../lib/formatError.js';
import { coinLogo } from '../lib/coinAssets.js';
import {
  COIN_CONFIG,
  SWAP_L2_COINS,
  formatAmount,
  safeBigInt,
  type Coin,
  type WalletBalance,
} from '@/shared';
import { parseHumanAmount } from '../lib/amountInput.js';

const SWAP_COINS: readonly Coin[] = SWAP_L2_COINS;

interface PricesResp {
  priceDecimals: number;
  prices: Record<Coin, string>;
}

interface AmountCoin {
  amount: string;
  amountHuman: string;
  coin: string;
}

interface NetworkFeeItem {
  type: string;
  amount: string;
  asset: string;
}

interface QuoteRoute {
  routeId: string;
  provider: string;
  providers: string[];
  tags: string[];
  youPay: AmountCoin;
  youReceive: AmountCoin;
  minReceive: AmountCoin;
  fees: {
    network: NetworkFeeItem[];
    platform: {
      bps: number;
      amount: string;
      amountHuman: string;
      asset: string;
      label: string;
    };
    totalPlatformBps: number;
  };
  etaSeconds?: {
    inbound?: number;
    swap?: number;
    outbound?: number;
    total?: number;
  } | null;
  txHint?: string | null;
  source: string;
}

interface QuoteListResp {
  routes: QuoteRoute[];
  providerErrors?: unknown;
}

interface SwapHistoryItem {
  id: string;
  fromCoin: Coin;
  toCoin: Coin;
  fromAmount: string;
  toAmount: string;
  feeAmount: string;
  feeBps: number;
  status: string;
  provider: string;
  createdAt: string;
  inboundTx?: string | null;
  outboundTx?: string | null;
  source: string;
}

interface ExecuteResp {
  id: string;
  fromAmount: string;
  toAmount: string;
  feeAmount: string;
  status: string;
  provider: string;
  source: string;
}

function formatEta(total?: number | null): string {
  if (total == null) return '—';
  if (total <= 0) return 'Instantâneo';
  if (total < 60) return `${total}s`;
  if (total < 3600) return `${Math.round(total / 60)} min`;
  return `${(total / 3600).toFixed(1)} h`;
}

function statusLabel(status: string): { text: string; cls: string } {
  switch (status) {
    case 'COMPLETED':
      return { text: 'Concluído', cls: 'bg-emerald-500/10 text-emerald-700' };
    case 'REFUNDED':
      return { text: 'Reembolsado', cls: 'bg-amber-500/10 text-amber-800' };
    case 'FAILED':
      return { text: 'Falhou', cls: 'bg-rose-500/10 text-rose-700' };
    case 'IN_FLIGHT':
    case 'BROADCASTING':
    case 'CREDITING':
    case 'LOCKED':
      return { text: 'Em andamento', cls: 'bg-sky-500/10 text-sky-700' };
    default:
      return { text: status, cls: 'bg-surface text-ink-muted' };
  }
}

export function SwapPage() {
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();

  const [fromCoin, setFromCoin] = useState<Coin>('POL');
  const [toCoin, setToCoin] = useState<Coin>('USDT');
  const [inputVal, setInputVal] = useState('');
  const [selectedRouteId, setSelectedRouteId] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ type: 'ok' | 'err'; text: string } | null>(null);

  const user = useAuthStore((s) => s.user);

  const walletsQ = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<{ wallets: WalletBalance[] }>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
  });

  const pricesQ = useQuery({
    queryKey: ['swap-prices'],
    queryFn: () => api<PricesResp>('/swap/prices', { skipAuth: true }),
    refetchInterval: 15_000,
  });

  const historyQ = useQuery({
    queryKey: ['swap-history'],
    queryFn: () => api<{ swaps: SwapHistoryItem[] }>('/swap/history'),
    enabled: Boolean(user),
    refetchInterval: 8_000,
  });

  const walletsMap = useMemo(() => {
    const m: Partial<Record<Coin, WalletBalance>> = {};
    const list = Array.isArray(walletsQ.data) ? walletsQ.data : walletsQ.data?.wallets ?? [];
    list.forEach((w) => (m[w.coin as Coin] = w));
    return m;
  }, [walletsQ.data]);

  const available = walletsMap[fromCoin] ? safeBigInt(walletsMap[fromCoin]!.balance) : 0n;
  const fromCfg = COIN_CONFIG[fromCoin] || COIN_CONFIG.POL;
  const toCfg = COIN_CONFIG[toCoin] || COIN_CONFIG.USDT;

  const smallestAmount = useMemo(() => parseHumanAmount(inputVal, fromCoin), [inputVal, fromCoin]);

  const quoteQ = useQuery({
    queryKey: ['swap-quote', fromCoin, toCoin, smallestAmount.toString()],
    queryFn: () =>
      api<QuoteListResp>(
        `/swap/quote?fromCoin=${fromCoin}&toCoin=${toCoin}&fromAmount=${smallestAmount.toString()}`,
        { skipAuth: true },
      ),
    enabled: smallestAmount > 0n && fromCoin !== toCoin,
    refetchInterval: 20_000,
    staleTime: 10_000,
  });

  const routes = quoteQ.data?.routes ?? [];
  const selected =
    routes.find((r) => r.routeId === selectedRouteId) ??
    routes.find((r) => r.source === 'swapkit') ??
    routes.find((r) => r.tags?.includes('RECOMMENDED')) ??
    routes[0] ??
    null;

  useEffect(() => {
    if (routes.length && (!selectedRouteId || !routes.some((r) => r.routeId === selectedRouteId))) {
      const pref =
        routes.find((r) => r.source === 'swapkit') ??
        routes.find((r) => r.tags?.includes('RECOMMENDED')) ??
        routes[0];
      setSelectedRouteId(pref?.routeId ?? null);
    }
  }, [routes, selectedRouteId]);

  const fromPriceScaled = pricesQ.data?.prices?.[fromCoin] ? safeBigInt(pricesQ.data.prices[fromCoin]!) : 0n;
  const toPriceScaled = pricesQ.data?.prices?.[toCoin] ? safeBigInt(pricesQ.data.prices[toCoin]!) : 0n;
  const priceDecimals = pricesQ.data?.priceDecimals ?? 8;

  const fromUsdVal = useMemo(() => {
    if (smallestAmount <= 0n || fromPriceScaled <= 0n) return null;
    const num =
      (Number(smallestAmount) / 10 ** fromCfg.decimals) * (Number(fromPriceScaled) / 10 ** priceDecimals);
    return num > 0 ? num.toLocaleString('en-US', { style: 'currency', currency: 'USD' }) : null;
  }, [smallestAmount, fromCfg.decimals, fromPriceScaled, priceDecimals]);

  const toUsdVal = useMemo(() => {
    if (!selected || toPriceScaled <= 0n) return null;
    const amt = safeBigInt(selected.youReceive.amount);
    const num = (Number(amt) / 10 ** toCfg.decimals) * (Number(toPriceScaled) / 10 ** priceDecimals);
    return num > 0 ? num.toLocaleString('en-US', { style: 'currency', currency: 'USD' }) : null;
  }, [selected, toCfg.decimals, toPriceScaled, priceDecimals]);

  const executeMutation = useMutation({
    mutationFn: () => {
      if (!selected) throw new Error('Selecione uma rota');
      return api<ExecuteResp>('/swap', {
        method: 'POST',
        json: {
          fromCoin,
          toCoin,
          fromAmount: smallestAmount.toString(),
          minToAmount: selected.minReceive.amount,
          expectedToAmount: selected.youReceive.amount,
          platformFeeBps: selected.fees.platform.bps,
          routeId: selected.routeId,
          provider: selected.provider,
          source: selected.source,
          idempotencyKey: crypto.randomUUID(),
        },
      });
    },
    onSuccess: (res) => {
      const instant = res.status === 'COMPLETED' || res.source === 'house';
      setMsg({
        type: 'ok',
        text: instant
          ? t('swap.success', { defaultValue: 'Swap concluído! Saldo creditado.' })
          : t('swap.pending', {
              defaultValue: `Swap enviado via ${res.provider}. Acompanhe o status no histórico.`,
            }),
      });
      setInputVal('');
      qc.invalidateQueries({ queryKey: ['wallets'] });
      qc.invalidateQueries({ queryKey: ['ledger'] });
      qc.invalidateQueries({ queryKey: ['swap-history'] });
    },
    onError: (e) => setMsg({ type: 'err', text: formatApiError(e) }),
  });

  useEffect(() => {
    if (fromCoin === toCoin) {
      const next = SWAP_COINS.find((c) => c !== fromCoin);
      if (next) setToCoin(next);
    }
  }, [fromCoin, toCoin]);

  function flip() {
    setFromCoin(toCoin);
    setToCoin(fromCoin);
    setInputVal('');
    setMsg(null);
    setSelectedRouteId(null);
  }

  const setPercentage = (pct: number) => {
    if (available <= 0n) return;
    const target = (available * BigInt(pct)) / 100n;
    setInputVal(formatAmount(target, fromCoin));
  };

  const insufficient = smallestAmount > available && available > 0n;
  const canSubmit =
    !!selected && !insufficient && !executeMutation.isPending && smallestAmount > 0n && !quoteQ.isFetching;

  const dateFmt = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? 'en', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

  const platformPct = selected ? (selected.fees.platform.bps / 100).toFixed(2) : '—';

  return (
    <div className="space-y-6 max-w-6xl mx-auto pb-16">
      <header>
        <div className="mb-1.5 inline-flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-xs font-black text-emerald-700">
          <i className="bi bi-arrow-left-right" />
          <span>SatsPay Instant Swap</span>
        </div>
        <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">
          {t('swap.title', { defaultValue: 'Câmbio & Swap de Criptomoedas' })}
        </h1>
        <p className="text-xs sm:text-sm text-ink-muted mt-1">
          {t('swap.subtitle', {
            defaultValue:
              'Swap DEX na Polygon (POL, USDT, USDC). Sem liquidez interna — rota externa via SwapKit.',
          })}
        </p>
      </header>

      <div className="grid gap-6 lg:grid-cols-12">
        <section className="lg:col-span-7">
          <motion.form
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            onSubmit={(e) => {
              e.preventDefault();
              if (canSubmit) executeMutation.mutate();
            }}
            className="rounded-3xl border border-border bg-paper p-6 sm:p-7 shadow-xs space-y-4"
          >
            <div className="rounded-2xl border border-border bg-surface p-4 sm:p-5 transition-all focus-within:border-emerald-500/60 focus-within:ring-2 focus-within:ring-emerald-500/10">
              <div className="mb-2.5 flex items-center justify-between text-xs">
                <span className="font-black uppercase tracking-wider text-ink-muted flex items-center gap-1.5">
                  <i className="bi bi-box-arrow-up-right text-emerald-600" />
                  {t('swap.from', { defaultValue: 'Você Envia' })}
                </span>
                <div className="flex items-center gap-2">
                  <span className="text-ink-muted text-xs">Disponível:</span>
                  <span className="font-mono font-bold text-ink">
                    {formatAmount(available, fromCoin)} {fromCoin}
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-3">
                <CoinPicker
                  value={fromCoin}
                  onChange={(c) => {
                    setFromCoin(c);
                    setMsg(null);
                    setSelectedRouteId(null);
                  }}
                  exclude={toCoin}
                />
                <div className="w-full text-right">
                  <input
                    type="text"
                    className="w-full border-none bg-transparent text-right font-mono text-2xl sm:text-3xl font-black outline-none placeholder:text-ink-muted/50 text-ink"
                    placeholder="0.00"
                    value={inputVal}
                    onChange={(e) => {
                      const val = e.target.value.replace(/[^0-9.]/g, '');
                      if ((val.match(/\./g) || []).length <= 1) setInputVal(val);
                    }}
                  />
                  {fromUsdVal && (
                    <div className="text-[11px] font-mono font-semibold text-ink-muted tracking-tight">
                      ≈ {fromUsdVal}
                    </div>
                  )}
                </div>
              </div>

              <div className="mt-3 flex items-center justify-between pt-2.5 border-t border-border/50">
                <span className="text-[11px] font-bold text-ink-muted">Quantia rápida:</span>
                <div className="flex items-center gap-1.5">
                  {[25, 50, 75, 100].map((pct) => (
                    <button
                      key={pct}
                      type="button"
                      onClick={() => setPercentage(pct)}
                      className="rounded-lg bg-paper px-2.5 py-1 text-[11px] font-bold text-ink-muted hover:bg-emerald-500/10 hover:text-emerald-700 transition-all border border-border active:scale-95 shadow-2xs"
                    >
                      {pct === 100 ? 'MÁXIMO' : `${pct}%`}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            <div className="relative my-2 flex justify-center">
              <div className="absolute inset-0 flex items-center" aria-hidden="true">
                <div className="w-full border-t border-border" />
              </div>
              <button
                type="button"
                onClick={flip}
                className="relative flex h-11 w-11 items-center justify-center rounded-2xl border-2 border-border bg-paper shadow-md transition-all hover:scale-110 hover:border-emerald-500 active:rotate-180 z-10"
                title={t('swap.flip', { defaultValue: 'Inverter moedas' })}
              >
                <i className="bi bi-arrow-down-up text-emerald-600 text-sm font-black" />
              </button>
            </div>

            <div className="rounded-2xl border border-border bg-surface p-4 sm:p-5">
              <div className="mb-2.5 flex items-center justify-between text-xs">
                <span className="font-black uppercase tracking-wider text-ink-muted flex items-center gap-1.5">
                  <i className="bi bi-box-arrow-in-down-left text-blue-600" />
                  {t('swap.to', { defaultValue: 'Você Recebe' })}
                </span>
                <span className="text-[11px] font-bold text-emerald-600 bg-emerald-500/10 px-2 py-0.5 rounded-full">
                  {selected?.source === 'house' ? 'Instantâneo' : formatEta(selected?.etaSeconds?.total)}
                </span>
              </div>

              <div className="flex items-center gap-3">
                <CoinPicker
                  value={toCoin}
                  onChange={(c) => {
                    setToCoin(c);
                    setMsg(null);
                    setSelectedRouteId(null);
                  }}
                  exclude={fromCoin}
                />
                <div className="w-full text-right">
                  <div className="font-mono text-2xl sm:text-3xl font-black text-ink truncate">
                    {selected ? selected.youReceive.amountHuman : quoteQ.isFetching ? '…' : '0.00'}
                  </div>
                  {toUsdVal && (
                    <div className="text-[11px] font-mono font-semibold text-emerald-600 tracking-tight">
                      ≈ {toUsdVal}
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* Route picker */}
            {routes.length > 1 && (
              <div className="space-y-2">
                <div className="text-[11px] font-black uppercase tracking-wider text-ink-muted">Rotas disponíveis</div>
                <div className="grid gap-2 sm:grid-cols-2">
                  {routes.map((r) => {
                    const active = selected?.routeId === r.routeId;
                    return (
                      <button
                        key={r.routeId}
                        type="button"
                        onClick={() => setSelectedRouteId(r.routeId)}
                        className={`rounded-2xl border p-3 text-left transition-all ${
                          active
                            ? 'border-emerald-500 bg-emerald-500/10 ring-2 ring-emerald-500/20'
                            : 'border-border bg-surface hover:border-emerald-500/40'
                        }`}
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-xs font-black text-ink truncate">{r.provider}</span>
                          {r.tags?.[0] && (
                            <span className="text-[9px] font-bold uppercase text-emerald-700 bg-emerald-500/10 px-1.5 py-0.5 rounded">
                              {r.tags[0]}
                            </span>
                          )}
                        </div>
                        <div className="mt-1 font-mono text-sm font-bold text-ink">
                          {r.youReceive.amountHuman} {toCoin}
                        </div>
                        <div className="mt-0.5 text-[10px] text-ink-muted">
                          Taxa SatsPay {r.fees.platform.bps / 100}% · ETA {formatEta(r.etaSeconds?.total)}
                        </div>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Transparent fee breakdown */}
            {selected && (
              <div className="space-y-2.5 rounded-2xl border border-emerald-500/30 bg-emerald-500/5 p-4 text-xs">
                <div className="flex items-center justify-between">
                  <span className="text-ink-muted font-medium">Provedor</span>
                  <span className="font-black text-ink">{selected.provider}</span>
                </div>
                {selected.fees.network.length > 0 ? (
                  selected.fees.network.map((f, i) => (
                    <div key={`${f.type}-${i}`} className="flex items-center justify-between">
                      <span className="text-ink-muted font-medium capitalize">Taxa de rede ({f.type})</span>
                      <span className="font-mono text-ink font-semibold">
                        {f.amount} {f.asset || ''}
                      </span>
                    </div>
                  ))
                ) : (
                  <div className="flex items-center justify-between">
                    <span className="text-ink-muted font-medium">Taxa de rede</span>
                    <span className="font-mono text-ink font-semibold">
                      {selected.source === 'house' ? '0 (pool interno)' : 'incluída no provedor'}
                    </span>
                  </div>
                )}
                <div className="flex items-center justify-between">
                  <span className="text-ink-muted font-medium">
                    {selected.fees.platform.label} ({platformPct}%)
                  </span>
                  <span className="font-mono text-ink font-semibold">
                    {selected.fees.platform.amountHuman} {selected.fees.platform.asset}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-ink-muted font-medium">Tempo estimado</span>
                  <span className="font-mono text-ink font-semibold">
                    {formatEta(selected.etaSeconds?.total)}
                  </span>
                </div>
                <div className="flex items-center justify-between border-t border-emerald-500/20 pt-2 font-bold">
                  <span className="text-ink">{t('swap.receive', { defaultValue: 'Você recebe (estimado)' })}</span>
                  <span className="font-mono text-lg font-black text-emerald-700">
                    +{selected.youReceive.amountHuman} {toCoin}
                  </span>
                </div>
                <p className="text-[10px] text-ink-muted leading-relaxed">
                  Taxas de rede/provedor (exceto inbound) já estão refletidas no valor a receber. A taxa SatsPay
                  é cobrada via affiliate do provedor ({selected.fees.platform.bps} bps).
                </p>
              </div>
            )}

            {quoteQ.isError && (
              <div className="rounded-2xl bg-rose-50 p-3.5 text-xs font-semibold text-rose-700 border border-rose-200">
                {formatApiError(quoteQ.error)}
              </div>
            )}

            {insufficient && (
              <div className="rounded-2xl bg-rose-50 p-3.5 text-xs font-semibold text-rose-700 border border-rose-200 flex items-center gap-2.5">
                <i className="bi bi-exclamation-triangle-fill text-base shrink-0" />
                <span>{t('swap.insufficient', { defaultValue: 'Saldo insuficiente na carteira.' })}</span>
              </div>
            )}

            {msg && (
              <div
                className={`rounded-2xl p-4 text-xs font-semibold border flex items-center gap-3 ${
                  msg.type === 'ok'
                    ? 'bg-emerald-50 text-emerald-800 border-emerald-200'
                    : 'bg-rose-50 text-rose-800 border-rose-200'
                }`}
              >
                <i
                  className={`bi ${
                    msg.type === 'ok'
                      ? 'bi-check-circle-fill text-emerald-600 text-lg'
                      : 'bi-exclamation-triangle-fill text-rose-600 text-lg'
                  } shrink-0`}
                />
                <span>{msg.text}</span>
              </div>
            )}

            <button
              type="submit"
              disabled={!canSubmit}
              className={`btn-primary w-full py-3.5 text-sm font-black flex items-center justify-center gap-2 shadow-md ${
                !canSubmit ? 'opacity-50 cursor-not-allowed' : 'hover:scale-[1.01] active:scale-[0.99]'
              }`}
            >
              {executeMutation.isPending ? (
                <>
                  <i className="bi bi-arrow-repeat animate-spin text-base" />
                  <span>{t('swap.confirming', { defaultValue: 'Processando swap...' })}</span>
                </>
              ) : (
                <>
                  <i className="bi bi-arrow-left-right text-base" />
                  <span>
                    {t('swap.confirm', { defaultValue: 'Confirmar Swap' })}
                    {selected ? ` · ${selected.provider}` : ''}
                  </span>
                </>
              )}
            </button>

            <div className="flex items-center justify-center gap-2 pt-1 text-center text-[11px] font-bold text-ink-muted">
              <i className="bi bi-shield-lock-fill text-emerald-600" />
              <span>
                {t('swap.trustLine', {
                  defaultValue: 'Custodial · Rota DEX (SwapKit) · Taxa transparente',
                })}
              </span>
            </div>
          </motion.form>
        </section>

        <section className="lg:col-span-5 space-y-3">
          <div className="flex items-center justify-between px-1">
            <h2 className="text-sm font-black uppercase tracking-wider text-ink flex items-center gap-2">
              <i className="bi bi-clock-history text-emerald-600" />
              <span>{t('swap.history', { defaultValue: 'Histórico de Swaps' })}</span>
            </h2>
            <span className="text-[11px] font-bold text-ink-muted font-mono">
              {historyQ.data?.swaps?.length ?? 0} registros
            </span>
          </div>

          <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
            {historyQ.isLoading ? (
              <div className="p-5 space-y-3">
                {Array.from({ length: 4 }).map((_, i) => (
                  <div key={i} className="h-14 rounded-2xl bg-surface animate-pulse" />
                ))}
              </div>
            ) : !historyQ.data || historyQ.data.swaps.length === 0 ? (
              <div className="p-10 text-center space-y-2">
                <div className="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-2xl bg-surface text-xl text-ink-muted border border-border">
                  <i className="bi bi-arrow-left-right" />
                </div>
                <h3 className="text-sm font-bold text-ink">
                  {t('swap.empty', { defaultValue: 'Nenhum swap realizado ainda' })}
                </h3>
              </div>
            ) : (
              <ul className="max-h-[520px] divide-y divide-border overflow-y-auto [scrollbar-width:none]">
                {historyQ.data.swaps.map((s) => {
                  const st = statusLabel(s.status);
                  return (
                    <li key={s.id} className="p-4 hover:bg-surface/50 transition-colors space-y-1.5">
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex items-center gap-2 min-w-0">
                          <img src={coinLogo(s.fromCoin)} alt={s.fromCoin} className="h-5 w-5 rounded-full object-contain" />
                          <span className="font-mono text-xs font-bold text-ink truncate">
                            {formatAmount(s.fromAmount, s.fromCoin)} {s.fromCoin}
                          </span>
                          <i className="bi bi-arrow-right text-xs text-ink-muted" />
                          <img src={coinLogo(s.toCoin)} alt={s.toCoin} className="h-5 w-5 rounded-full object-contain" />
                          <span className="font-mono text-xs font-black text-emerald-600 truncate">
                            +{formatAmount(s.toAmount, s.toCoin)} {s.toCoin}
                          </span>
                        </div>
                        <span className={`shrink-0 rounded-full px-2 py-0.5 text-[9px] font-black uppercase ${st.cls}`}>
                          {st.text}
                        </span>
                      </div>
                      <div className="flex items-center justify-between text-[10px] text-ink-muted pt-0.5">
                        <span className="font-mono">{dateFmt.format(new Date(s.createdAt))}</span>
                        <span className="font-mono truncate ml-2">
                          {s.provider} · {s.feeBps / 100}%
                        </span>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function CoinPicker({
  value,
  onChange,
  exclude,
}: {
  value: Coin;
  onChange: (c: Coin) => void;
  exclude?: Coin;
}) {
  const [open, setOpen] = useState(false);

  return (
    <div className="relative shrink-0">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 rounded-2xl bg-paper px-3.5 py-2 shadow-xs border border-border hover:border-emerald-500 transition-all hover:scale-105 active:scale-95"
      >
        <img src={coinLogo(value)} alt={value} className="h-6 w-6 rounded-full object-contain" />
        <span className="text-sm font-black text-ink">{value}</span>
        <i className="bi bi-chevron-down text-xs text-ink-muted" />
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-20" onClick={() => setOpen(false)} />
          <div className="absolute left-0 top-full z-30 mt-1.5 max-h-72 w-56 overflow-y-auto rounded-2xl border border-border bg-paper p-1.5 shadow-2xl [scrollbar-width:none]">
            <div className="px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-ink-muted">
              Selecione a Criptomoeda
            </div>
            {SWAP_COINS.filter((c) => c !== exclude).map((c) => (
              <button
                type="button"
                key={c}
                onClick={() => {
                  onChange(c);
                  setOpen(false);
                }}
                className={`flex w-full items-center gap-2.5 rounded-xl px-3 py-2 text-xs transition-colors hover:bg-surface ${
                  c === value ? 'bg-emerald-500/10 font-black text-emerald-700' : 'text-ink font-semibold'
                }`}
              >
                <img src={coinLogo(c)} alt={c} className="h-5 w-5 rounded-full object-contain" />
                <span>{c}</span>
                <span className="ml-auto text-[10px] text-ink-muted truncate">{COIN_CONFIG[c]?.name}</span>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

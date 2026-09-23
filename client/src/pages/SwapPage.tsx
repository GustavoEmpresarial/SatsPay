import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useSearchParams } from 'react-router-dom';
import { motion } from 'framer-motion';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { formatApiError } from '../lib/formatError.js';
import { coinLogo } from '../lib/coinAssets.js';
import {
  COIN_CONFIG,
  coinsForMode,
  coinNetwork,
  defaultPairForMode,
  formatAmount,
  isBridgePair,
  isDexSwapPair,
  isSameSwapNetwork,
  safeBigInt,
  type Coin,
  type SwapMode,
  type WalletBalance,
} from '@/shared';
import { parseHumanAmount } from '../lib/amountInput.js';

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
  error?: string | null;
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

/** SwapKit assets: `POL.USDT-0xabc…` → `USDT`. */
function shortAsset(asset: string): string {
  const leaf = (asset.split('.').pop() || asset).trim();
  return leaf.split('-')[0] || leaf;
}

function pickPreferredRoute(routes: QuoteRoute[]): QuoteRoute | null {
  if (!routes.length) return null;
  return [...routes].sort((a, b) => {
    const diff = safeBigInt(b.youReceive.amount) - safeBigInt(a.youReceive.amount);
    if (diff > 0n) return 1;
    if (diff < 0n) return -1;
    return 0;
  })[0]!;
}

function isNoRoutesError(err: unknown): boolean {
  if (err && typeof err === 'object' && 'code' in err) {
    const code = String((err as { code?: unknown }).code ?? '');
    if (code === 'NO_ROUTES') return true;
  }
  const msg = formatApiError(err).toLowerCase();
  return (
    msg.includes('no routes') ||
    msg.includes('no route') ||
    msg.includes('sem rota') ||
    msg.includes('routes available')
  );
}

function feeTypeLabel(type: string): string {
  switch (type.toLowerCase()) {
    case 'deposit':
      return 'Depósito ChangeNOW (fix)';
    case 'liquidity':
      return 'Liquidez do DEX';
    case 'service':
      return 'Serviço do provedor';
    case 'inbound':
      return 'Gas de entrada';
    case 'outbound':
      return 'Gas de saída';
    case 'relayerservice':
      return 'Serviço Relay (solver)';
    case 'relayergas':
      return 'Gas do Relay';
    case 'relayer':
      return 'Taxa Relay (total)';
    case 'gas':
      return 'Gas da origem';
    case 'app':
      return 'App fee Relay';
    case 'subsidized':
      return 'Subsídio Relay';
    default:
      return `Taxa (${type})`;
  }
}

function trimFeeAmount(raw: string): string {
  if (!raw.includes('.')) return raw;
  const trimmed = raw.replace(/(\.\d*?[1-9])0+$/, '$1').replace(/\.0+$/, '');
  return trimmed || raw;
}

function isSwapInFlight(status: string): boolean {
  return (
    status === 'IN_FLIGHT' ||
    status === 'BROADCASTING' ||
    status === 'CREDITING' ||
    status === 'LOCKED' ||
    status === 'PENDING' ||
    status === 'QUEUED'
  );
}

function isSwapTerminal(status: string): boolean {
  return status === 'COMPLETED' || status === 'REFUNDED' || status === 'FAILED';
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
    case 'PENDING':
    case 'QUEUED':
      return { text: 'Em andamento', cls: 'bg-sky-500/10 text-sky-700' };
    default:
      return { text: status, cls: 'bg-surface text-ink-muted' };
  }
}

/// `dex_swaps.error` carries raw provider/RPC text (relay status codes,
/// JSON-RPC error bodies) meant for support, not for the history list — same
/// principle as not showing the raw AMOUNT_TOO_LOW body on execute. This
/// buckets it into a short reason so a refund/failure never reads as a bare
/// status pill with zero context.
function explainSwapError(raw: string | null | undefined): string | null {
  if (!raw) return null;
  const lower = raw.toLowerCase();
  if (lower.startsWith('slippage')) {
    return 'O preço mudou durante a confirmação e ficou fora do mínimo combinado.';
  }
  if (lower.startsWith('broadcast failed')) {
    return 'Falha ao enviar a transação na rede de origem.';
  }
  if (lower.includes('relay status') || lower.includes('changenow status')) {
    return 'O provedor da rota não conseguiu concluir a operação.';
  }
  return 'A operação não pôde ser concluída.';
}

export function SwapPage() {
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();

  const mode: SwapMode = searchParams.get('tab') === 'bridge' ? 'bridge' : 'swap';
  const modeCoins = coinsForMode(mode);

  const [fromCoin, setFromCoin] = useState<Coin>(() => defaultPairForMode(mode).from);
  const [toCoin, setToCoin] = useState<Coin>(() => defaultPairForMode(mode).to);
  const [inputVal, setInputVal] = useState('');
  const [selectedRouteId, setSelectedRouteId] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ type: 'ok' | 'err'; text: string } | null>(null);
  const [feeDetailsOpen, setFeeDetailsOpen] = useState(false);

  const switchMode = (next: SwapMode) => {
    const pair = defaultPairForMode(next);
    setFromCoin(pair.from);
    setToCoin(pair.to);
    setInputVal('');
    setMsg(null);
    setSelectedRouteId(null);
    setFeeDetailsOpen(false);
    const p = new URLSearchParams(searchParams);
    if (next === 'bridge') p.set('tab', 'bridge');
    else p.delete('tab');
    setSearchParams(p, { replace: true });
  };

  // Keep pair valid when landing with ?tab= or flipping coins.
  useEffect(() => {
    const ok =
      mode === 'swap' ? isDexSwapPair(fromCoin, toCoin) : isBridgePair(fromCoin, toCoin);
    if (ok) return;
    const pair = defaultPairForMode(mode);
    setFromCoin(pair.from);
    setToCoin(pair.to);
    setSelectedRouteId(null);
  }, [mode, fromCoin, toCoin]);

  const toPickerCoins = useMemo(() => {
    if (mode === 'swap') return modeCoins.filter((c) => c !== fromCoin);
    return modeCoins.filter((c) => c !== fromCoin && !isSameSwapNetwork(fromCoin, c));
  }, [mode, modeCoins, fromCoin]);

  const fromPickerCoins = useMemo(() => modeCoins, [modeCoins]);

  const user = useAuthStore((s) => s.user);

  const historyQ = useQuery({
    queryKey: ['swap-history'],
    queryFn: () => api<{ swaps: SwapHistoryItem[] }>('/swap/history'),
    enabled: Boolean(user),
    refetchInterval: 8_000,
  });

  const hasInFlightSwap = useMemo(
    () => (historyQ.data?.swaps ?? []).some((s) => isSwapInFlight(s.status)),
    [historyQ.data],
  );

  const walletsQ = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<{ wallets: WalletBalance[] }>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
    // Keep "Disponível" fresh while Relay/1inch is still settling.
    refetchInterval: hasInFlightSwap ? 8_000 : false,
  });

  const pricesQ = useQuery({
    queryKey: ['swap-prices'],
    queryFn: () => api<PricesResp>('/swap/prices', { skipAuth: true }),
    refetchInterval: 15_000,
  });

  // When Relay/1inch finishes async, history flips to COMPLETED but wallets were
  // only invalidated at submit — refresh balances without a full page reload.
  const prevSwapStatusRef = useRef<Map<string, string>>(new Map());
  useEffect(() => {
    const swaps = historyQ.data?.swaps;
    if (!swaps) return;

    const prev = prevSwapStatusRef.current;
    const next = new Map<string, string>();
    let settled = false;
    let completed = false;

    for (const s of swaps) {
      const before = prev.get(s.id);
      if (before && isSwapInFlight(before) && isSwapTerminal(s.status)) {
        settled = true;
        if (s.status === 'COMPLETED') completed = true;
      }
      next.set(s.id, s.status);
    }
    prevSwapStatusRef.current = next;

    if (!settled) return;
    void qc.invalidateQueries({ queryKey: ['wallets'] });
    void qc.invalidateQueries({ queryKey: ['ledger'] });
    if (completed) {
      setMsg({
        type: 'ok',
        text: t('swap.success', { defaultValue: 'Swap concluído! Saldo creditado.' }),
      });
    }
  }, [historyQ.data, historyQ.dataUpdatedAt, qc, t]);

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

  const pairValid =
    mode === 'swap' ? isDexSwapPair(fromCoin, toCoin) : isBridgePair(fromCoin, toCoin);

  const quoteQ = useQuery({
    queryKey: ['swap-quote', mode, fromCoin, toCoin, smallestAmount.toString()],
    queryFn: () =>
      api<QuoteListResp>(
        `/swap/quote?fromCoin=${fromCoin}&toCoin=${toCoin}&fromAmount=${smallestAmount.toString()}`,
        { skipAuth: true },
      ),
    enabled: pairValid && smallestAmount > 0n && fromCoin !== toCoin,
    refetchInterval: 20_000,
    staleTime: 10_000,
  });

  const routes = useMemo(() => quoteQ.data?.routes ?? [], [quoteQ.data]);
  const selected =
    routes.find((r) => r.routeId === selectedRouteId) ?? pickPreferredRoute(routes);

  useEffect(() => {
    if (routes.length && (!selectedRouteId || !routes.some((r) => r.routeId === selectedRouteId))) {
      setSelectedRouteId(pickPreferredRoute(routes)?.routeId ?? null);
    }
  }, [routes, selectedRouteId]);

  const fromPriceScaled = pricesQ.data?.prices?.[fromCoin] ? safeBigInt(pricesQ.data.prices[fromCoin]!) : 0n;
  const toPriceScaled = pricesQ.data?.prices?.[toCoin] ? safeBigInt(pricesQ.data.prices[toCoin]!) : 0n;
  const priceDecimals = pricesQ.data?.priceDecimals ?? 8;

  const usd = (n: number | null) =>
    n !== null && n > 0 ? n.toLocaleString('en-US', { style: 'currency', currency: 'USD' }) : null;

  const fromUsdNum = useMemo(() => {
    if (smallestAmount <= 0n || fromPriceScaled <= 0n) return null;
    return (Number(smallestAmount) / 10 ** fromCfg.decimals) * (Number(fromPriceScaled) / 10 ** priceDecimals);
  }, [smallestAmount, fromCfg.decimals, fromPriceScaled, priceDecimals]);

  const toUsdNum = useMemo(() => {
    if (!selected || toPriceScaled <= 0n) return null;
    const amt = safeBigInt(selected.youReceive.amount);
    return (Number(amt) / 10 ** toCfg.decimals) * (Number(toPriceScaled) / 10 ** priceDecimals);
  }, [selected, toCfg.decimals, toPriceScaled, priceDecimals]);

  const fromUsdVal = usd(fromUsdNum);
  const toUsdVal = usd(toUsdNum);

  /// Route costs (relayer, gas, DEX spread) are mostly fixed, so on small
  /// amounts they eat a large share of the value. The quote still succeeds —
  /// nothing warns the user they are burning most of what they send.
  const valueLossWarning = useMemo(() => {
    if (fromUsdNum === null || toUsdNum === null) return null;
    if (fromUsdNum <= 0 || toUsdNum <= 0) return null;
    const pct = ((fromUsdNum - toUsdNum) / fromUsdNum) * 100;
    if (pct < 10) return null;
    return { pct, from: usd(fromUsdNum), to: usd(toUsdNum), severe: pct >= 25 };
  }, [fromUsdNum, toUsdNum]);

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
      const next = toPickerCoins[0];
      if (next) setToCoin(next);
    }
  }, [fromCoin, toCoin, toPickerCoins]);

  function flip() {
    if (mode === 'bridge' && isSameSwapNetwork(toCoin, fromCoin)) return;
    // Bridge flip must stay cross-network; swap flip always stays on Polygon.
    if (mode === 'bridge' && !isBridgePair(toCoin, fromCoin)) return;
    if (mode === 'swap' && !isDexSwapPair(toCoin, fromCoin)) return;
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
    pairValid &&
    !!selected &&
    !insufficient &&
    !executeMutation.isPending &&
    smallestAmount > 0n &&
    !quoteQ.isFetching;

  const dateFmt = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? 'en', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

  const platformPct = selected ? (selected.fees.platform.bps / 100).toFixed(2) : '—';

  const depositFeeWarning = useMemo(() => {
    if (!selected?.fees.network?.length || smallestAmount <= 0n) return null;
    const dep = selected.fees.network.find((f) => f.type.toLowerCase() === 'deposit');
    if (!dep) return null;
    const feeLedger = parseHumanAmount(dep.amount, fromCoin);
    if (feeLedger <= 0n) return null;
    const pct = Number((feeLedger * 10000n) / smallestAmount) / 100;
    if (pct < 8) return null;
    return { pct, amountHuman: trimFeeAmount(dep.amount), asset: shortAsset(dep.asset || fromCoin) };
  }, [selected, smallestAmount, fromCoin]);

  const historyItems = useMemo(() => {
    const all = historyQ.data?.swaps ?? [];
    return all.filter((s) =>
      mode === 'swap' ? isSameSwapNetwork(s.fromCoin, s.toCoin) : !isSameSwapNetwork(s.fromCoin, s.toCoin),
    );
  }, [historyQ.data, mode]);

  const noRoutes = useMemo(() => {
    if (!pairValid || smallestAmount <= 0n || quoteQ.isFetching) return false;
    if (routes.length > 0) return false;
    if (quoteQ.isError) return isNoRoutesError(quoteQ.error);
    // Success with empty routes (defensive).
    return quoteQ.isSuccess;
  }, [pairValid, smallestAmount, quoteQ.isFetching, quoteQ.isError, quoteQ.isSuccess, quoteQ.error, routes.length]);

  const quoteOtherError = quoteQ.isError && !isNoRoutesError(quoteQ.error);

  return (
    <div className="space-y-6 max-w-6xl mx-auto pb-16">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <div className="mb-1.5 inline-flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-xs font-black text-emerald-700">
            <i className={`bi ${mode === 'swap' ? 'bi-arrow-left-right' : 'bi-globe2'}`} />
            <span>{mode === 'swap' ? 'SatsPay Instant Swap' : 'SatsPay Bridge'}</span>
          </div>
          <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">
            {mode === 'swap'
              ? t('swap.title', { defaultValue: 'Swap' })
              : t('swap.bridgeTitle', { defaultValue: 'Bridge' })}
          </h1>
          <p className="text-xs sm:text-sm text-ink-muted mt-1 max-w-2xl">
            {mode === 'swap'
              ? t('swap.subtitleSwap', {
                  defaultValue:
                    'Troca na mesma rede. Na SatsPay: POL, USDT e USDC na Polygon (DEX / 1inch).',
                })
              : t('swap.subtitleBridge', {
                  defaultValue:
                    'Cruza redes distintas: Solana ↔ Polygon (Relay) ou L1 nativa ↔ outra moeda (ChangeNOW).',
                })}
          </p>
        </div>

        <div className="inline-flex rounded-2xl border border-border bg-surface p-1 shadow-xs self-start">
          <button
            type="button"
            onClick={() => switchMode('swap')}
            className={`inline-flex items-center gap-2 rounded-xl px-4 py-2.5 text-xs font-bold transition-all ${
              mode === 'swap'
                ? 'bg-emerald-600 text-white shadow-md shadow-emerald-600/20'
                : 'text-ink-muted hover:text-ink hover:bg-paper'
            }`}
          >
            <i className="bi bi-arrow-left-right text-sm" />
            <span>{t('swap.tabSwap', { defaultValue: 'Swap' })}</span>
          </button>
          <button
            type="button"
            onClick={() => switchMode('bridge')}
            className={`inline-flex items-center gap-2 rounded-xl px-4 py-2.5 text-xs font-bold transition-all ${
              mode === 'bridge'
                ? 'bg-emerald-600 text-white shadow-md shadow-emerald-600/20'
                : 'text-ink-muted hover:text-ink hover:bg-paper'
            }`}
          >
            <i className="bi bi-globe2 text-sm" />
            <span>{t('swap.tabBridge', { defaultValue: 'Bridge' })}</span>
          </button>
        </div>
      </header>

      <div className="grid gap-6 lg:grid-cols-12">
        <section className="lg:col-span-7">
          <motion.form
            key={mode}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            onSubmit={(e) => {
              e.preventDefault();
              if (canSubmit && pairValid) executeMutation.mutate();
            }}
            className="rounded-3xl border border-border bg-paper p-6 sm:p-7 shadow-xs space-y-4"
          >
            {mode === 'bridge' && (
              <div className="rounded-2xl border border-sky-500/30 bg-sky-500/10 px-3.5 py-2.5 text-[11px] leading-relaxed text-sky-950">
                <strong className="font-black">Bridge ≠ Swap.</strong>{' '}
                {t('swap.bridgeHint', {
                  defaultValue:
                    'Origem e destino em redes diferentes. Ex.: SOL (Solana) → USDT (Polygon), ou BTC → POL.',
                })}
              </div>
            )}
            {mode === 'swap' && (
              <div className="rounded-2xl border border-emerald-500/30 bg-emerald-500/10 px-3.5 py-2.5 text-[11px] leading-relaxed text-emerald-950">
                <strong className="font-black">Swap = mesma rede.</strong>{' '}
                {t('swap.swapHint', {
                  defaultValue: 'Só Polygon: POL ↔ USDT ↔ USDC. Para mudar de rede, use a aba Bridge.',
                })}
              </div>
            )}
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
                  coins={fromPickerCoins}
                  onChange={(c) => {
                    setFromCoin(c);
                    setMsg(null);
                    setSelectedRouteId(null);
                    if (mode === 'bridge' && isSameSwapNetwork(c, toCoin)) {
                      const alt = modeCoins.find((x) => x !== c && !isSameSwapNetwork(c, x));
                      if (alt) setToCoin(alt);
                    }
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
                  coins={toPickerCoins}
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

            {/* Resumo da cotação — linguagem clara, sem contratos 0x */}
            {selected && (
              <div className="overflow-hidden rounded-2xl border border-border bg-surface/60 text-xs">
                <div className="flex items-end justify-between gap-3 border-b border-border bg-paper/80 px-4 py-3.5">
                  <div>
                    <div className="text-[11px] font-medium text-ink-muted">
                      {t('swap.receive', { defaultValue: 'Você recebe' })}
                    </div>
                    <div className="mt-0.5 font-mono text-xl font-bold text-emerald-700">
                      +{selected.youReceive.amountHuman} {toCoin}
                    </div>
                    {toUsdVal && <div className="mt-0.5 font-mono text-[11px] text-ink-muted">≈ {toUsdVal}</div>}
                  </div>
                  <div className="text-right text-[11px] text-ink-muted">
                    <div className="font-semibold text-ink">via {selected.provider}</div>
                    <div>~{formatEta(selected.etaSeconds?.total)}</div>
                  </div>
                </div>

                <div className="space-y-2 px-4 py-3">
                  <div className="flex items-center justify-between gap-3">
                    <span className="text-ink-muted">Taxa SatsPay ({platformPct}%)</span>
                    <span className="font-mono font-semibold text-ink">
                      {trimFeeAmount(selected.fees.platform.amountHuman)}{' '}
                      {shortAsset(selected.fees.platform.asset || fromCoin)}
                    </span>
                  </div>

                  {depositFeeWarning && (
                    <div className="rounded-xl border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-900">
                      <div className="font-bold">Valor pequeno pra esta rota</div>
                      <div className="mt-0.5">
                        Depósito fixo do ChangeNOW ({depositFeeWarning.amountHuman}{' '}
                        {depositFeeWarning.asset}) ≈ <strong>{depositFeeWarning.pct.toFixed(0)}%</strong> do
                        que você envia — não é a taxa SatsPay (0,25%). Aumente o valor ou use outro par.
                      </div>
                    </div>
                  )}

                  {valueLossWarning && (
                    <div
                      role="alert"
                      className={`rounded-xl border px-3 py-2 text-[11px] ${
                        valueLossWarning.severe
                          ? 'border-rose-500/40 bg-rose-500/10 text-rose-900'
                          : 'border-amber-500/40 bg-amber-500/10 text-amber-900'
                      }`}
                    >
                      <div className="font-bold">
                        {t('swap.valueLossTitle', {
                          pct: valueLossWarning.pct.toFixed(0),
                          defaultValue: `Você perde ~${valueLossWarning.pct.toFixed(0)}% do valor nesta rota`,
                        })}
                      </div>
                      <div className="mt-0.5">
                        {t('swap.valueLossBody', {
                          from: valueLossWarning.from,
                          to: valueLossWarning.to,
                          defaultValue: `Você envia ${valueLossWarning.from} e recebe ${valueLossWarning.to}. Os custos de rota (relayer, gas, spread) são quase fixos, então em valores baixos eles consomem a maior parte. Aumentar a quantia costuma melhorar muito essa proporção.`,
                        })}
                      </div>
                    </div>
                  )}

                  <button
                    type="button"
                    onClick={() => setFeeDetailsOpen((v) => !v)}
                    className="flex w-full items-center justify-between rounded-lg py-1 text-left text-[11px] font-medium text-ink-muted hover:text-ink"
                  >
                    <span>
                      {feeDetailsOpen
                        ? 'Ocultar custos da rota'
                        : selected.fees.network.length > 0
                          ? `Custos da rota (${selected.fees.network.length})`
                          : 'Custos da rota'}
                    </span>
                    <i className={`bi bi-chevron-${feeDetailsOpen ? 'up' : 'down'} text-[10px]`} />
                  </button>

                  {feeDetailsOpen && (
                    <div className="space-y-2 rounded-xl border border-border bg-paper/70 px-3 py-2.5">
                      {selected.fees.network.length === 0 ? (
                        <div className="flex justify-between gap-3 text-ink-muted">
                          <span>Taxas de rede / provedor</span>
                          <span className="font-mono text-ink">não detalhadas nesta rota</span>
                        </div>
                      ) : (
                        selected.fees.network.map((f, i) => (
                          <div key={`${f.type}-${i}`} className="flex items-start justify-between gap-3">
                            <span className="text-ink-muted">{feeTypeLabel(f.type)}</span>
                            <span className="max-w-[55%] text-right font-mono font-semibold text-ink">
                              {trimFeeAmount(f.amount)} {shortAsset(f.asset || '')}
                            </span>
                          </div>
                        ))
                      )}
                      <p className="border-t border-border pt-2 text-[10px] leading-relaxed text-ink-muted">
                        {selected.source === 'relay'
                          ? 'Custos Relay (solver/gas) vêm na cotação. Em valores pequenos a taxa fixa do solver pode ser alta. “Você recebe” já desconta isso.'
                          : 'Esses custos vêm do provedor DEX. O valor “Você recebe” já desconta o que a rota cobrou (exceto gas de entrada, quando cobrado à parte).'}
                      </p>
                    </div>
                  )}

                  <p className="text-[10px] leading-relaxed text-ink-muted">
                    A taxa SatsPay ({platformPct}%) é só a comissão da plataforma. Custos Relay/DEX
                    aparecem acima e já estão embutidos no “Você recebe”.
                  </p>
                </div>
              </div>
            )}

            {noRoutes && (
              <div className="rounded-2xl border border-amber-500/35 bg-gradient-to-br from-amber-500/10 via-surface to-paper px-4 py-4 text-left shadow-xs">
                <div className="flex items-start gap-3">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl border border-amber-500/30 bg-paper text-amber-700">
                    <i className="bi bi-signpost-2 text-lg" />
                  </div>
                  <div className="min-w-0 space-y-1.5">
                    <div className="text-sm font-black text-ink">
                      {t('swap.noRoutesTitle', { defaultValue: 'Sem rota pra este par' })}
                    </div>
                    <p className="text-[12px] leading-relaxed text-ink-muted">
                      {mode === 'swap'
                        ? t('swap.noRoutesSwap', {
                            from: fromCoin,
                            to: toCoin,
                            defaultValue: `Não achamos liquidez DEX pra ${fromCoin} → ${toCoin} na Polygon agora. Tente outro par (POL, USDT ou USDC) ou um valor diferente.`,
                          })
                        : t('swap.noRoutesBridge', {
                            from: fromCoin,
                            to: toCoin,
                            fromNet: coinNetwork(fromCoin).short,
                            toNet: coinNetwork(toCoin).short,
                            defaultValue: `Nenhum provedor cotou ${fromCoin} (${coinNetwork(fromCoin).short}) → ${toCoin} (${coinNetwork(toCoin).short}) neste momento. Em bridges L1 o valor mínimo costuma ser maior — tente SOL ↔ USDT/USDC/POL, ou aumente a quantia.`,
                          })}
                    </p>
                    <p className="text-[10px] font-medium text-ink-muted/90">
                      {t('swap.noRoutesHint', {
                        defaultValue:
                          'Taxa SatsPay só aparece quando há rota. Isso não é saldo insuficiente.',
                      })}
                    </p>
                  </div>
                </div>
              </div>
            )}

            {quoteOtherError && (
              <div className="rounded-2xl bg-rose-50 p-3.5 text-xs font-semibold text-rose-700 border border-rose-200 flex items-center gap-2.5">
                <i className="bi bi-exclamation-triangle-fill text-base shrink-0" />
                <span>{formatApiError(quoteQ.error)}</span>
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
                  <i className={`bi ${mode === 'swap' ? 'bi-arrow-left-right' : 'bi-globe2'} text-base`} />
                  <span>
                    {selected
                      ? `${mode === 'swap' ? t('swap.confirm', { defaultValue: 'Trocar' }) : t('swap.confirmBridge', { defaultValue: 'Bridge' })} ${fromCoin} → ${toCoin}`
                      : mode === 'swap'
                        ? t('swap.confirm', { defaultValue: 'Confirmar swap' })
                        : t('swap.confirmBridge', { defaultValue: 'Confirmar bridge' })}
                  </span>
                </>
              )}
            </button>

            <div className="flex items-center justify-center gap-2 pt-1 text-center text-[11px] text-ink-muted">
              <i className="bi bi-shield-lock-fill text-emerald-600" />
              <span>
                {mode === 'swap'
                  ? t('swap.trustLineSwap', {
                      defaultValue: 'Swap Polygon · taxa SatsPay 0,25%',
                    })
                  : t('swap.trustLineBridge', {
                      defaultValue: 'Bridge cross-rede · taxa SatsPay 0,25%',
                    })}
              </span>
            </div>
          </motion.form>
        </section>

        <section className="lg:col-span-5 space-y-3">
          <div className="flex items-center justify-between px-1">
            <h2 className="text-sm font-black uppercase tracking-wider text-ink flex items-center gap-2">
              <i className="bi bi-clock-history text-emerald-600" />
              <span>
                {mode === 'swap'
                  ? t('swap.history', { defaultValue: 'Histórico de swaps' })
                  : t('swap.historyBridge', { defaultValue: 'Histórico de bridges' })}
              </span>
            </h2>
            <span className="text-[11px] font-bold text-ink-muted font-mono">
              {historyItems.length} registros
            </span>
          </div>

          <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
            {historyQ.isLoading ? (
              <div className="p-5 space-y-3">
                {Array.from({ length: 4 }).map((_, i) => (
                  <div key={i} className="h-14 rounded-2xl bg-surface animate-pulse" />
                ))}
              </div>
            ) : historyItems.length === 0 ? (
              <div className="p-10 text-center space-y-2">
                <div className="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-2xl bg-surface text-xl text-ink-muted border border-border">
                  <i className={`bi ${mode === 'swap' ? 'bi-arrow-left-right' : 'bi-globe2'}`} />
                </div>
                <h3 className="text-sm font-bold text-ink">
                  {mode === 'swap'
                    ? t('swap.empty', { defaultValue: 'Nenhum swap ainda.' })
                    : t('swap.emptyBridge', { defaultValue: 'Nenhum bridge ainda.' })}
                </h3>
              </div>
            ) : (
              <ul className="max-h-[520px] divide-y divide-border overflow-y-auto [scrollbar-width:none]">
                {historyItems.map((s) => {
                  const st = statusLabel(s.status);
                  const errorReason = (s.status === 'REFUNDED' || s.status === 'FAILED') ? explainSwapError(s.error) : null;
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
                          {coinNetwork(s.fromCoin).short}→{coinNetwork(s.toCoin).short} · {s.provider} ·{' '}
                          {s.feeBps / 100}%
                        </span>
                      </div>
                      {errorReason && (
                        <div className="text-[10px] text-amber-800/80">
                          {errorReason}
                          {s.status === 'REFUNDED' && ' Valor devolvido à sua carteira.'}
                        </div>
                      )}
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
  coins,
}: {
  value: Coin;
  onChange: (c: Coin) => void;
  exclude?: Coin;
  coins: readonly Coin[];
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const net = coinNetwork(value);
  const options = coins.filter((c) => c !== exclude);

  return (
    <div className="relative shrink-0">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 rounded-2xl bg-paper px-3.5 py-2 shadow-xs border border-border hover:border-emerald-500 transition-all hover:scale-105 active:scale-95"
        title={t('swap.networkOn', {
          network: net.label,
          defaultValue: `Rede SatsPay: ${net.label}`,
        })}
      >
        <img src={coinLogo(value)} alt={value} className="h-6 w-6 rounded-full object-contain" />
        <span className="flex flex-col items-start leading-tight">
          <span className="text-sm font-black text-ink">{value}</span>
          <span className="text-[9px] font-bold uppercase tracking-wide text-ink-muted">{net.short}</span>
        </span>
        <i className="bi bi-chevron-down text-xs text-ink-muted" />
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-20" onClick={() => setOpen(false)} />
          <div className="absolute left-0 top-full z-30 mt-1.5 max-h-72 w-64 overflow-y-auto rounded-2xl border border-border bg-paper p-1.5 shadow-2xl [scrollbar-width:none]">
            <div className="px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-ink-muted">
              {t('swap.pickCoin', { defaultValue: 'Selecione a criptomoeda' })}
            </div>
            {options.map((c) => {
              const n = coinNetwork(c);
              return (
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
                  <span className="flex flex-col items-start leading-tight min-w-0">
                    <span>{c}</span>
                    <span className="text-[9px] font-bold uppercase tracking-wide text-ink-muted truncate">
                      {n.short}
                    </span>
                  </span>
                  <span className="ml-auto text-[10px] text-ink-muted truncate max-w-[40%]">
                    {COIN_CONFIG[c]?.name}
                  </span>
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

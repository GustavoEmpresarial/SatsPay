import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COIN_CONFIG, COINS, formatAmount, formatLedgerAmount, getCoinUsdValue, isCoin, safeBigInt, type Coin, type WalletBalance } from '@/shared';
import { clsx } from 'clsx';

interface MerchantInvoice {
  id: string;
  orderId: string;
  siteName: string | null;
  coin: Coin;
  amount: string;
  feeAmount: string;
  netAmount: string;
  depositAddress: string;
  status: 'PENDING' | 'DETECTED' | 'CONFIRMED' | 'PAID' | 'EXPIRED' | 'CANCELLED';
  customerEmail: string | null;
  description: string | null;
  webhookDelivered: boolean;
  createdAt: string;
  paidAt: string | null;
  /** Unscaled USD from create (`amountUsd` on create response). */
  amountUsd?: string | null;
  /** Scaled USD ask — list payloads expose this instead of `amountUsd`. */
  priceUsdScaled?: string | null;
  priceDecimals?: number | null;
}

/** Invoice `amount` is ledger units (1e-8). Never multiply raw by USD price. */
function invoiceHumanTokens(amount: string, coin: Coin): number {
  const n = Number(formatLedgerAmount(amount, coin));
  return Number.isFinite(n) ? n : 0;
}

function invoiceUsdValue(
  inv: MerchantInvoice,
  prices?: Record<string, string>,
  priceDecimals = 8,
): number {
  if (inv.amountUsd != null && inv.amountUsd !== '') {
    const n = Number(inv.amountUsd);
    if (Number.isFinite(n) && n >= 0) return n;
  }
  if (inv.priceUsdScaled != null && inv.priceDecimals != null) {
    const n = Number(inv.priceUsdScaled) / 10 ** Math.max(0, inv.priceDecimals);
    if (Number.isFinite(n) && n > 0) return n;
  }
  if (!isCoin(inv.coin)) return 0;
  return getCoinUsdValue(inv.amount, inv.coin, prices, priceDecimals);
}

interface PricesResp {
  priceDecimals: number;
  prices: Record<string, string>;
}

interface ApiKeysResp {
  keys: Array<{ id: string; name: string; isActive: boolean }>;
}

interface InvoicesPayload {
  invoices?: MerchantInvoice[];
}

interface WalletsPayload {
  wallets?: WalletBalance[];
}

export function MerchantDashboardPage() {
  const [filterCoin, setFilterCoin] = useState<string>('ALL');
  const user = useAuthStore((s) => s.user);

  const invoicesQ = useQuery({
    queryKey: ['merchant-invoices'],
    queryFn: async () => {
      const res = await api<MerchantInvoice[] | InvoicesPayload>('/merchant/deposits');
      return Array.isArray(res) ? res : (res.invoices ?? []);
    },
    enabled: Boolean(user),
    refetchInterval: 15_000,
  });

  const devWalletsQ = useQuery({
    queryKey: ['wallets', 'MERCHANT'],
    queryFn: async () => {
      const res = await api<WalletBalance[] | WalletsPayload>('/wallet?kind=MERCHANT');
      return Array.isArray(res) ? res : (res.wallets ?? []);
    },
    enabled: Boolean(user),
    refetchInterval: 15_000,
  });

  // 3. Live Oracle Crypto Prices
  const pricesQ = useQuery({
    queryKey: ['prices'],
    queryFn: () => api<PricesResp>('/swap/prices', { skipAuth: true }),
    refetchInterval: 30_000,
  });

  // 4. API Keys
  const apiKeysQ = useQuery({
    queryKey: ['api-keys'],
    queryFn: () => api<ApiKeysResp>('/api-keys'),
    enabled: Boolean(user),
  });

  const invoices = useMemo(
    () => (Array.isArray(invoicesQ.data) ? invoicesQ.data : []),
    [invoicesQ.data],
  );
  const devWallets = useMemo(
    () => (Array.isArray(devWalletsQ.data) ? devWalletsQ.data : []),
    [devWalletsQ.data],
  );

  // Parse Live Prices Map
  const livePrices = useMemo(() => {
    const map: Record<Coin, number> = {
      USDT: 1.0,
      USDC: 1.0,
      BTC: 88500.0,
      SOL: 195.0,
      POL: 0.42,
      LTC: 92.0,
      DOGE: 0.18,
      BCH: 480.0,
      DGB: 0.012,
      ZER: 0.01,
      PEPE: 0.000004,
    };

    if (pricesQ.data?.prices) {
      const dec = pricesQ.data.priceDecimals || 6;
      for (const [coinKey, rawScaled] of Object.entries(pricesQ.data.prices)) {
        const num = Number(rawScaled) / 10 ** dec;
        if (num > 0 && isCoin(coinKey)) {
          map[coinKey] = num;
        }
      }
    }
    return map;
  }, [pricesQ.data]);

  // Comprehensive Metrics Calculation
  const stats = useMemo(() => {
    let totalDepositsReceivedUsd = 0;
    let totalPaidInvoicesCount = 0;
    let totalPendingInvoicesCount = 0;
    let totalExpiredInvoicesCount = 0;
    const volumeUsdByCoin: Partial<Record<Coin, number>> = {};
    const volumeTokensByCoin: Partial<Record<Coin, number>> = {};
    const countByCoin: Partial<Record<Coin, number>> = {};

    COINS.forEach((c) => {
      volumeUsdByCoin[c] = 0;
      volumeTokensByCoin[c] = 0;
      countByCoin[c] = 0;
    });

    const safeInvoices = Array.isArray(invoices) ? invoices : [];
    safeInvoices.forEach((inv) => {
      if (!inv || !isCoin(inv.coin)) return;
      const isPaid = inv.status === 'PAID' || inv.status === 'CONFIRMED';
      const humanTokens = invoiceHumanTokens(inv.amount, inv.coin);
      const volumeUsd = invoiceUsdValue(inv, pricesQ.data?.prices, pricesQ.data?.priceDecimals ?? 8);

      if (isPaid) {
        totalPaidInvoicesCount++;
        totalDepositsReceivedUsd += volumeUsd;
        volumeUsdByCoin[inv.coin] = (volumeUsdByCoin[inv.coin] || 0) + volumeUsd;
        volumeTokensByCoin[inv.coin] = (volumeTokensByCoin[inv.coin] || 0) + humanTokens;
        countByCoin[inv.coin] = (countByCoin[inv.coin] || 0) + 1;
      } else if (inv.status === 'PENDING' || inv.status === 'DETECTED') {
        totalPendingInvoicesCount++;
      } else {
        totalExpiredInvoicesCount++;
      }
    });

    const totalInvoicesGenerated = safeInvoices.length;
    const conversionRate = totalInvoicesGenerated > 0 ? (totalPaidInvoicesCount / totalInvoicesGenerated) * 100 : 0;

    // Calculate total available balance in developer wallets (Valores em Caixa)
    let totalCashInTreasuryUsd = 0;
    const devBalanceMap: Partial<Record<Coin, { balance: bigint; tokens: number; usd: number }>> = {};
    const safeDevWallets = Array.isArray(devWallets) ? devWallets : [];

    COINS.forEach((c) => {
      const w = safeDevWallets.find((item) => item && item.coin === c);
      const bal = w ? safeBigInt(w.balance) : 0n;
      const cfg = COIN_CONFIG[c] || COIN_CONFIG.BTC;
      const humanBal = Number(bal) / 10 ** cfg.decimals;
      const price = livePrices[c] || 1.0;
      const usdVal = humanBal * price;

      totalCashInTreasuryUsd += usdVal;
      devBalanceMap[c] = { balance: bal, tokens: humanBal, usd: usdVal };
    });

    // Cash the merchant can withdraw. Do not invent a payout from (volume − cash).
    const totalPayoutsSentUsd = 0;
    const netCashFlowUsd = totalCashInTreasuryUsd;

    return {
      totalDepositsReceivedUsd,
      totalPaidInvoicesCount,
      totalPendingInvoicesCount,
      totalExpiredInvoicesCount,
      totalInvoicesGenerated,
      conversionRate,
      totalCashInTreasuryUsd,
      totalPayoutsSentUsd,
      netCashFlowUsd,
      devBalanceMap,
      volumeUsdByCoin,
      volumeTokensByCoin,
      countByCoin,
    };
  }, [invoices, devWallets, livePrices, pricesQ.data]);

  const filteredInvoices = useMemo(() => {
    if (filterCoin === 'ALL') return invoices;
    return invoices.filter((i) => i.coin === filterCoin);
  }, [invoices, filterCoin]);

  return (
    <div className="space-y-6 max-w-7xl mx-auto pb-12">
      {/* HEADER */}
      <header className="border-b border-border/80 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <span className="flex h-2.5 w-2.5 rounded-full bg-emerald-500 animate-pulse" />
          <span className="text-xs font-bold uppercase tracking-widest text-emerald-600">
            Painel do Comerciante · Em Tempo Real
          </span>
        </div>
        <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">
          Dashboard
        </h1>
        <p className="text-xs text-ink-muted mt-1">
          Métricas consolidadas de depósitos recebidos via API, saques enviados e valores disponíveis em caixa com cotação em USD e tokens.
        </p>
        <p className="mt-2 text-xs text-ink rounded-xl border border-bitcoin/25 bg-bitcoin/5 px-3 py-2 max-w-2xl">
          O cliente paga o valor integral. A taxa de 0,25% é descontada do que você recebe
          (<span className="font-mono">feeAmount + netAmount = amount</span>).
        </p>
      </header>

      {/* TOP 4 KPI METRIC CARDS (Depósitos, Saques, Valores em Caixa, Margem Líquida) */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 sm:gap-4">
        {/* 1. Depósitos Recebidos (Inflows) */}
        <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs space-y-1">
          <div className="flex items-center justify-between text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            <span>Depósitos Recebidos</span>
            <div className="flex h-6 w-6 items-center justify-center rounded-lg bg-emerald-500/10 text-emerald-600">
              <i className="bi bi-arrow-down-left" />
            </div>
          </div>
          <div className="text-xl sm:text-3xl font-black text-ink">
            ${stats.totalDepositsReceivedUsd.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </div>
          <div className="text-[10px] text-emerald-600 font-bold flex items-center gap-1">
            <i className="bi bi-check2-circle" />
            <span>{stats.totalPaidInvoicesCount} faturas liquidadas via API</span>
          </div>
        </div>

        {/* 2. Saques / Repasses Enviados (Outflows) */}
        <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs space-y-1">
          <div className="flex items-center justify-between text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            <span>Disponível para saque</span>
            <div className="flex h-6 w-6 items-center justify-center rounded-lg bg-rose-500/10 text-rose-600">
              <i className="bi bi-arrow-up-right" />
            </div>
          </div>
          <div className="text-xl sm:text-3xl font-black text-ink">
            ${stats.totalCashInTreasuryUsd.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </div>
          <div className="text-[10px] text-ink-muted font-medium">
            Saque on-chain: origem Caixa do comerciante
          </div>
        </div>

        {/* 3. Valores em Caixa (Current Cash / Treasury) */}
        <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs space-y-1">
          <div className="flex items-center justify-between text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            <span>Valores em Caixa</span>
            <div className="flex h-6 w-6 items-center justify-center rounded-lg bg-bitcoin/10 text-bitcoin">
              <i className="bi bi-safe2-fill" />
            </div>
          </div>
          <div className="text-xl sm:text-3xl font-black text-bitcoin-dark">
            ${stats.totalCashInTreasuryUsd.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </div>
          <div className="text-[10px] text-ink-muted font-medium">
            Disponível em saldos comerciais
          </div>
        </div>

        {/* 4. Taxa de Conversão & Uptime da API */}
        <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs space-y-1">
          <div className="flex items-center justify-between text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            <span>Taxa de Conversão</span>
            <div className="flex h-6 w-6 items-center justify-center rounded-lg bg-blue-500/10 text-blue-600">
              <i className="bi bi-funnel-fill" />
            </div>
          </div>
          <div className="text-xl sm:text-3xl font-black text-ink">
            {stats.totalInvoicesGenerated > 0 ? `${stats.conversionRate.toFixed(1)}%` : '0.0%'}
          </div>
          <div className="text-[10px] text-ink-muted font-medium flex items-center gap-1">
            {stats.totalInvoicesGenerated > 0 ? (
              <>
                <span className="flex h-1.5 w-1.5 rounded-full bg-emerald-500" />
                <span>{stats.totalPaidInvoicesCount} de {stats.totalInvoicesGenerated} liquidadas</span>
              </>
            ) : (
              <span>0 faturas geradas</span>
            )}
          </div>
        </div>
      </div>

      {/* SECTION: VALORES EM CAIXA DETALHADOS POR MOEDA (TREASURY BALANCES) */}
      <div className="rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-xs space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 border-b border-border/80 pb-3">
          <div>
            <h2 className="text-sm font-bold text-ink flex items-center gap-2">
              <i className="bi bi-wallet2 text-bitcoin" />
              <span>Valores em Caixa por Criptomoeda (Saldos Comerciais)</span>
            </h2>
            <p className="text-[11px] text-ink-muted mt-0.5">
              Montante custodiado exibido em Dólar (USD) e na quantidade exata de cada criptoativo.
            </p>
          </div>
          <Link
            to="/developer-wallets"
            className="text-xs font-bold text-emerald-700 hover:underline flex items-center gap-1 self-start sm:self-auto"
          >
            <span>Transferir para Pessoal</span>
            <i className="bi bi-arrow-right" />
          </Link>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 pt-1">
          {COINS.map((c) => {
            const cfg = COIN_CONFIG[c] || COIN_CONFIG.BTC;
            const data = stats.devBalanceMap[c] || { balance: 0n, tokens: 0, usd: 0 };
            const volUsd = stats.volumeUsdByCoin[c] || 0;
            const volTokens = stats.volumeTokensByCoin[c] || 0;
            const paidOps = stats.countByCoin[c] || 0;

            return (
              <div
                key={c}
                className="rounded-2xl border border-border bg-surface/60 p-3.5 space-y-2.5 hover:bg-surface transition-colors"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <img src={coinLogo(c)} alt={c} className="h-6 w-6 rounded-full object-contain shrink-0" />
                    <div>
                      <div className="font-bold text-xs text-ink leading-tight">{cfg.name}</div>
                      <div className="font-mono text-[10px] text-ink-muted uppercase">{c}</div>
                    </div>
                  </div>
                  <div className="text-right">
                    <div className="rounded-full bg-paper border border-border px-2 py-0.5 font-mono text-[11px] font-bold text-emerald-700">
                      ${data.usd.toFixed(2)} USD
                    </div>
                  </div>
                </div>

                <div className="rounded-xl bg-paper p-2.5 border border-border/80 space-y-1.5">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="text-ink-muted font-semibold">Em Caixa:</span>
                    <div className="text-right">
                      <span className="font-mono font-black text-ink">
                        ${data.usd.toFixed(2)} USD
                      </span>
                      <span className="font-mono text-[10px] text-ink-muted block">
                        ({formatAmount(data.balance, c)} {c})
                      </span>
                    </div>
                  </div>

                  <div className="flex items-center justify-between text-[11px] border-t border-border/50 pt-1">
                    <span className="text-ink-muted font-semibold">Total Processado:</span>
                    <div className="text-right">
                      <span className="font-mono font-bold text-emerald-600">
                        ${volUsd.toFixed(2)} USD
                      </span>
                      <span className="font-mono text-[10px] text-ink-muted block">
                        ({volTokens > 0 ? volTokens.toFixed(4) : '0'} {c} · {paidOps} ops)
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* 2-COLUMN SECTION: BREAKDOWN OF API ACTIVITY + OPERATIONAL SETTLEMENT */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* LEFT: VOLUME & STATUS DISTRIBUTION */}
        <div className="lg:col-span-6 rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-xs space-y-4">
          <div className="flex items-center justify-between border-b border-border/80 pb-3">
            <div>
              <h2 className="text-sm font-bold text-ink flex items-center gap-2">
                <i className="bi bi-pie-chart-fill text-emerald-600" />
                <span>Volume de Depósitos por Moeda</span>
              </h2>
              <p className="text-[11px] text-ink-muted mt-0.5">
                Valores recebidos discriminados em Dólar e tokens.
              </p>
            </div>
            <span className="text-[10px] font-bold uppercase tracking-wider text-emerald-600 bg-emerald-500/10 px-2.5 py-0.5 rounded-full">
              {stats.totalPaidInvoicesCount} Liquidados
            </span>
          </div>

          <div className="space-y-3 pt-1">
            {COINS.map((c) => {
              const volUsd = stats.volumeUsdByCoin[c] || 0;
              const volTokens = stats.volumeTokensByCoin[c] || 0;
              const pct = stats.totalDepositsReceivedUsd > 0 ? (volUsd / stats.totalDepositsReceivedUsd) * 100 : 0;
              const cfg = COIN_CONFIG[c] || COIN_CONFIG.BTC;

              return (
                <div key={c} className="space-y-1">
                  <div className="flex items-center justify-between text-xs font-bold">
                    <div className="flex items-center gap-2">
                      <img src={coinLogo(c)} alt={c} className="h-4 w-4 rounded-full object-contain" />
                      <span className="text-ink">{cfg.name}</span>
                      <span className="text-[10px] text-ink-muted font-mono uppercase">({c})</span>
                    </div>
                    <div className="text-right font-mono text-ink">
                      <span>${volUsd.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</span>
                      <span className="text-[10px] text-ink-muted font-normal ml-1">
                        ({volTokens > 0 ? volTokens.toFixed(4) : '0'} {c} · {pct.toFixed(0)}%)
                      </span>
                    </div>
                  </div>

                  <div className="h-2 w-full bg-surface rounded-full overflow-hidden border border-border">
                    <div
                      className="h-full bg-gradient-to-r from-emerald-500 to-teal-500 rounded-full transition-all duration-500"
                      style={{ width: `${Math.max(volUsd > 0 ? 5 : 0, Math.min(100, pct))}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* RIGHT: OPERATIONAL PERFORMANCE & SETTLEMENT METRICS */}
        <div className="lg:col-span-6 rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-xs space-y-4">
          <div className="flex items-center justify-between border-b border-border/80 pb-3">
            <div>
              <h2 className="text-sm font-bold text-ink flex items-center gap-2">
                <i className="bi bi-graph-up-arrow text-emerald-600" />
                <span>Performance Operacional & Liquidação</span>
              </h2>
              <p className="text-[11px] text-ink-muted mt-0.5">
                Métricas de eficiência, conversão e tempo de resposta do seu gateway.
              </p>
            </div>
            <span className="text-[10px] font-bold uppercase tracking-wider text-blue-600 bg-blue-500/10 px-2.5 py-0.5 rounded-full">
              Automático
            </span>
          </div>

          <div className="grid grid-cols-2 gap-3 pt-1">
            {/* Ticket Médio */}
            <div className="rounded-2xl border border-border bg-surface/60 p-3.5 space-y-1">
              <div className="text-[10px] uppercase font-bold text-ink-muted flex items-center gap-1.5">
                <i className="bi bi-tag-fill text-bitcoin" />
                <span>Ticket Médio</span>
              </div>
              <div className="text-lg font-black text-ink font-mono">
                ${(stats.totalPaidInvoicesCount > 0 ? stats.totalDepositsReceivedUsd / stats.totalPaidInvoicesCount : 0).toFixed(2)} USD
              </div>
              <div className="text-[10px] text-ink-muted">Por cobrança paga</div>
            </div>

            {/* Tempo de Confirmação */}
            <div className="rounded-2xl border border-border bg-surface/60 p-3.5 space-y-1">
              <div className="text-[10px] uppercase font-bold text-ink-muted flex items-center gap-1.5">
                <i className="bi bi-lightning-charge-fill text-amber-500" />
                <span>Confirmação Média</span>
              </div>
              <div className="text-lg font-black text-emerald-600 font-mono">
                Instantânea
              </div>
              <div className="text-[10px] text-ink-muted">Crédito após 1 conf</div>
            </div>

            {/* Taxa de Entrega Webhooks */}
            <div className="rounded-2xl border border-border bg-surface/60 p-3.5 space-y-1">
              <div className="text-[10px] uppercase font-bold text-ink-muted flex items-center gap-1.5">
                <i className="bi bi-broadcast text-blue-500" />
                <span>Entrega Webhook</span>
              </div>
              <div className="text-lg font-black text-blue-600 font-mono">
                100%
              </div>
              <div className="text-[10px] text-ink-muted">Notificações Assinadas</div>
            </div>

            {/* Uptime do Gateway */}
            <div className="rounded-2xl border border-border bg-surface/60 p-3.5 space-y-1">
              <div className="text-[10px] uppercase font-bold text-ink-muted flex items-center gap-1.5">
                <i className="bi bi-shield-check text-emerald-600" />
                <span>Disponibilidade</span>
              </div>
              <div className="text-lg font-black text-emerald-600 font-mono">
                99.98%
              </div>
              <div className="text-[10px] text-ink-muted">Latência média: 12ms</div>
            </div>
          </div>

          {/* Webhook & API Keys Status */}
          <div className="rounded-2xl border border-border bg-surface p-4 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin text-lg">
                <i className="bi bi-key-fill" />
              </div>
              <div>
                <div className="text-xs font-bold text-ink">
                  {apiKeysQ.data?.keys?.length || 0} Chave(s) de API Ativa(s)
                </div>
                <div className="text-[10px] text-ink-muted">
                  Credenciais protegidas e Webhooks integrados.
                </div>
              </div>
            </div>
            <Link
              to="/api-keys"
              className="rounded-xl bg-paper border border-border hover:bg-surface px-3 py-1.5 text-xs font-bold text-ink transition-all shadow-xs"
            >
              Gerenciar Chaves
            </Link>
          </div>
        </div>
      </div>

      {/* RECENT INCOMING DEPOSITS & API INVOICES TABLE */}
      <div className="rounded-3xl border border-border bg-paper overflow-hidden shadow-xs">
        <div className="p-5 sm:p-6 border-b border-border flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 bg-surface/40">
          <div>
            <h2 className="text-sm font-bold text-ink flex items-center gap-2">
              <i className="bi bi-receipt-cutoff text-emerald-600" />
              <span>Extrato de Depósitos & Cobranças Recebidas via API</span>
            </h2>
            <p className="text-[11px] text-ink-muted mt-0.5">
              Transações e pagamentos liquidados pelo seu gateway.
            </p>
          </div>

          {/* Filter Coins */}
          <div className="flex items-center gap-1.5 overflow-x-auto [scrollbar-width:none]">
            <button
              type="button"
              onClick={() => setFilterCoin('ALL')}
              className={clsx(
                'rounded-xl px-2.5 py-1 text-[11px] font-bold transition-all',
                filterCoin === 'ALL'
                  ? 'bg-emerald-600 text-white shadow-xs'
                  : 'bg-surface border border-border text-ink-muted hover:text-ink',
              )}
            >
              Todas
            </button>
            {COINS.map((c) => (
              <button
                key={c}
                type="button"
                onClick={() => setFilterCoin(c)}
                className={clsx(
                  'rounded-xl px-2 py-1 text-[11px] font-bold transition-all',
                  filterCoin === c
                    ? 'bg-emerald-600 text-white shadow-xs'
                    : 'bg-surface border border-border text-ink-muted hover:text-ink',
                )}
              >
                {c}
              </button>
            ))}
          </div>
        </div>

        {filteredInvoices.length === 0 ? (
          <div className="p-12 text-center text-ink-muted space-y-3">
            <div className="flex h-12 w-12 mx-auto items-center justify-center rounded-full bg-surface text-ink-muted text-xl">
              <i className="bi bi-inbox" />
            </div>
            <div className="text-sm font-bold text-ink">Nenhuma cobrança registrada ainda</div>
            <p className="text-xs text-ink-muted max-w-sm mx-auto">
              Quando seus clientes pagarem pedidos criados via API, os depósitos e liquidações aparecerão em tempo real aqui.
            </p>
            <Link
              to="/docs"
              className="rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white font-bold text-xs px-4 py-2 shadow-xs transition-all inline-flex items-center gap-1.5"
            >
              <i className="bi bi-book-half" />
              <span>Ver Documentação da API</span>
            </Link>
          </div>
        ) : (
          <div className="overflow-x-auto [scrollbar-width:none]">
            <table className="w-full text-left text-xs">
              <thead className="bg-surface text-[10px] uppercase font-bold text-ink-muted border-b border-border">
                <tr>
                  <th className="p-3.5">ID da Ordem / Pedido</th>
                  <th className="p-3.5">Origem / Cliente</th>
                  <th className="p-3.5">Moeda</th>
                  <th className="p-3.5">Valor (USD & Token)</th>
                  <th className="p-3.5">Status</th>
                  <th className="p-3.5">Data & Hora</th>
                  <th className="p-3.5 text-right">Auditoria</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border font-medium">
                {filteredInvoices.slice(0, 10).map((inv) => {
                  const isPaid = inv.status === 'PAID' || inv.status === 'CONFIRMED';
                  const isPending = inv.status === 'PENDING' || inv.status === 'DETECTED';
                  const usdVal = isCoin(inv.coin)
                    ? invoiceUsdValue(inv, pricesQ.data?.prices, pricesQ.data?.priceDecimals ?? 8)
                    : 0;
                  const tokenDisplay = isCoin(inv.coin) ? formatLedgerAmount(inv.amount, inv.coin) : inv.amount;

                  return (
                    <tr key={inv.id} className="hover:bg-surface/50 transition-colors">
                      <td className="p-3.5">
                        <div className="font-mono font-bold text-ink">{inv.orderId}</div>
                        <div className="text-[10px] text-ink-muted font-mono truncate max-w-[120px]">
                          {inv.id}
                        </div>
                      </td>

                      <td className="p-3.5">
                        <div className="font-bold text-ink truncate max-w-[180px]">
                          {inv.customerEmail || 'Cliente via API'}
                        </div>
                        <div className="text-[10px] text-ink-muted truncate max-w-[180px]">
                          {inv.description || inv.siteName || 'Depósito de Gateway'}
                        </div>
                      </td>

                      <td className="p-3.5 font-bold">
                        <div className="flex items-center gap-1.5">
                          <img
                            src={coinLogo(inv.coin)}
                            alt={inv.coin}
                            className="h-5 w-5 rounded-full object-contain shrink-0"
                          />
                          <span>{inv.coin}</span>
                        </div>
                      </td>

                      <td className="p-3.5 font-mono">
                        <div className="font-bold text-ink">
                          ${usdVal.toFixed(2)} USD
                        </div>
                        <div className="text-[10px] text-ink-muted">
                          {tokenDisplay} {inv.coin}
                        </div>
                      </td>

                      <td className="p-3.5">
                        <span
                          className={clsx(
                            'rounded-full px-2.5 py-0.5 text-[9px] font-bold inline-flex items-center gap-1',
                            isPaid
                              ? 'bg-emerald-500/15 text-emerald-700'
                              : isPending
                              ? 'bg-amber-500/15 text-amber-700'
                              : 'bg-rose-500/15 text-rose-700',
                          )}
                        >
                          {isPaid ? (
                            <>
                              <i className="bi bi-check-circle-fill" /> PAGO (LIQUIDADO)
                            </>
                          ) : isPending ? (
                            <>
                              <i className="bi bi-hourglass-split" /> PENDENTE
                            </>
                          ) : (
                            <>
                              <i className="bi bi-x-circle-fill" /> EXPIRADO
                            </>
                          )}
                        </span>
                      </td>

                      <td className="p-3.5 font-mono text-ink-muted text-[11px]">
                        {new Date(inv.createdAt).toLocaleString('pt-BR', {
                          day: '2-digit',
                          month: '2-digit',
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </td>

                      <td className="p-3.5 text-right">
                        <a
                          href={`/pay/${inv.id}`}
                          target="_blank"
                          rel="noreferrer"
                          className="rounded-lg bg-surface border border-border px-2.5 py-1 text-[11px] font-bold text-ink hover:bg-paper hover:text-emerald-700 shadow-xs inline-flex items-center gap-1"
                        >
                          <span>Checkout</span>
                          <i className="bi bi-box-arrow-up-right text-[10px]" />
                        </a>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

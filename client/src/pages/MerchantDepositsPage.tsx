import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COINS, formatLedgerAmount, toLedgerUnits, type Coin } from '@/shared';
import { CodeBlock, EndpointHeader, MultiLangCodeBlock } from '../components/ApiSnippets.js';
import { clsx } from 'clsx';

interface InvoiceItem {
  id: string;
  orderId: string;
  siteUserId?: string;
  siteName?: string;
  coin: Coin;
  amount: string;
  feeAmount: string;
  netAmount: string;
  depositAddress: string;
  status: 'PENDING' | 'DETECTED' | 'CONFIRMED' | 'PAID' | 'EXPIRED' | 'CANCELLED';
  callbackUrl: string;
  customerEmail?: string;
  webhookDelivered: boolean;
  webhookStatusCode?: number;
  webhookAttempts: number;
  txHash?: string;
  paidAt?: string;
  createdAt: string;
}

declare global {
  interface Window {
    SatsPay?: { renderButtons?: (root?: ParentNode) => number };
  }
}

interface WebhookTestResult {
  delivered?: boolean;
  statusCode?: number;
  error?: string;
}

/** Absolute origin for copy-paste snippets. */
const ORIGIN = typeof window !== 'undefined' && window.location?.origin
  ? window.location.origin
  : 'https://www.satspay.pro';

export function MerchantDepositsPage() {
  const qc = useQueryClient();
  const [filterCoin, setFilterCoin] = useState<string>('ALL');
  const [filterStatus, setFilterStatus] = useState<string>('ALL');
  const [testResult, setTestResult] = useState<{ id: string; msg: string } | null>(null);

  // Preview rendered by the real SDK, so what the merchant sees here is
  // literally what their customer gets — a hand-built imitation would drift
  // from the script the moment either changed.
  const buttonPreviewRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const SRC = `${ORIGIN}/sdk/satspay-pay.js`;
    const render = () => window.SatsPay?.renderButtons?.(buttonPreviewRef.current ?? undefined);

    if (window.SatsPay?.renderButtons) {
      render();
      return;
    }
    const existing = document.querySelector<HTMLScriptElement>(`script[src="${SRC}"]`);
    if (existing) {
      existing.addEventListener('load', render, { once: true });
      return;
    }
    const script = document.createElement('script');
    script.src = SRC;
    script.async = true;
    script.addEventListener('load', render, { once: true });
    document.head.appendChild(script);
  }, []);

  const { data, isLoading } = useQuery<{ invoices: InvoiceItem[] }>({
    queryKey: ['merchant-deposits'],
    queryFn: () => api('/merchant/deposits'),
    refetchInterval: 5000,
  });

  // Coins this merchant accepts on the hosted checkout. The old panel with
  // this name persisted nothing; this one round-trips to the server.
  const settingsQ = useQuery<{ acceptedCoins: Coin[]; availableCoins: Coin[] }>({
    queryKey: ['merchant-settings'],
    queryFn: () => api('/merchant/settings'),
  });
  const acceptedCoins = settingsQ.data?.acceptedCoins ?? [];
  const availableCoins = settingsQ.data?.availableCoins ?? [];

  const saveSettingsMut = useMutation({
    mutationFn: (coins: Coin[]) =>
      api('/merchant/settings', { method: 'PUT', json: { acceptedCoins: coins } }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['merchant-settings'] }),
  });

  const toggleAccepted = (c: Coin) => {
    const next = acceptedCoins.includes(c)
      ? acceptedCoins.filter((x) => x !== c)
      : [...acceptedCoins, c];
    // The API refuses a selection with nothing payable in it; do not even
    // send that request.
    if (next.length === 0) return;
    saveSettingsMut.mutate(next);
  };

  const testWebhookMut = useMutation({
    mutationFn: (invId: string) =>
      api<WebhookTestResult>(`/merchant/deposits/${invId}/test-webhook`, { method: 'POST' }),
    onSuccess: (res, invId) => {
      qc.invalidateQueries({ queryKey: ['merchant-deposits'] });
      setTestResult({
        id: invId,
        msg: res.delivered
          ? `✅ Webhook entregue com sucesso! (Status ${res.statusCode})`
          : `⚠️ Falha ao entregar webhook: ${res.error || 'Timeout ou erro no site de destino'}`,
      });
      setTimeout(() => setTestResult(null), 5000);
    },
  });

  const invoices = data?.invoices ?? [];

  const filteredInvoices = invoices.filter((inv) => {
    if (filterCoin !== 'ALL' && inv.coin !== filterCoin) return false;
    if (filterStatus !== 'ALL') {
      if (filterStatus === 'PAID' && inv.status !== 'CONFIRMED' && inv.status !== 'PAID') return false;
      if (filterStatus === 'PENDING' && inv.status !== 'PENDING' && inv.status !== 'DETECTED') return false;
      if (filterStatus === 'EXPIRED' && inv.status !== 'EXPIRED' && inv.status !== 'CANCELLED') return false;
    }
    return true;
  });

  const confirmedCount = invoices.filter((i) => i.status === 'CONFIRMED' || i.status === 'PAID').length;
  const pendingCount = invoices.filter((i) => i.status === 'PENDING' || i.status === 'DETECTED').length;

  return (
    <div className="space-y-6 max-w-7xl mx-auto pb-12">
      {/* HEADER */}
      <header className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4 border-b border-border/80 pb-5">
        <div>
          <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-emerald-600 mb-1">
            <span className="flex h-2.5 w-2.5 rounded-full bg-emerald-500 animate-pulse" />
            <span>Gateway de Cobranças & Depósitos</span>
          </div>
          <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">
            Cobranças & Faturas da API
          </h1>
          <p className="text-xs sm:text-sm text-ink-muted mt-1">
            Histórico completo de faturas geradas via API REST com monitoramento de confirmações blockchain e entrega de webhooks (IPN).
          </p>
        </div>

        {/* Cohesive Action Toolbar — one size for all four */}
        <div className="w-full lg:w-auto">
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 p-1.5 rounded-2xl bg-surface border border-border shadow-2xs">
            {[
              { to: '/deposit', icon: 'bi-qr-code', accent: 'text-bitcoin', label: 'Endereços de Depósito' },
              { to: '/api-keys', icon: 'bi-key-fill', accent: 'text-amber-500', label: 'Chaves de API' },
              { to: '/pay/demo', icon: 'bi-eye-fill', accent: 'text-purple-600', label: 'Ver Checkout (Demo)' },
              { to: '/docs?tab=deposits', icon: 'bi-book-half', accent: 'text-emerald-600', label: 'Documentação' },
            ].map((b) => (
              <Link
                key={b.to}
                to={b.to}
                className="flex h-full items-center justify-center gap-2 rounded-xl bg-paper hover:bg-surface-hover active:scale-[0.98] px-3 py-2.5 text-center text-xs font-bold text-ink border border-border/60 transition-all shadow-2xs"
              >
                <i className={clsx('bi', b.icon, b.accent)} />
                <span>{b.label}</span>
              </Link>
            ))}
          </div>
        </div>
      </header>

      {/* STATS OVERVIEW CARDS */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 sm:gap-4">
        <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs space-y-1">
          <div className="text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            Total de Cobranças
          </div>
          <div className="text-2xl sm:text-3xl font-black text-ink">{invoices.length}</div>
          <div className="text-[10px] text-ink-muted">Faturas geradas pela API</div>
        </div>

        <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs space-y-1">
          <div className="text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            Depósitos Liquidados
          </div>
          <div className="text-2xl sm:text-3xl font-black text-emerald-600">
            {confirmedCount}
          </div>
          <div className="text-[10px] text-emerald-700/80 font-bold">Creditados na carteira comercial</div>
        </div>

        <div className="rounded-3xl border border-border bg-paper p-4 sm:p-5 shadow-xs space-y-1">
          <div className="text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            Pendentes / Aguardando
          </div>
          <div className="text-2xl sm:text-3xl font-black text-amber-600">
            {pendingCount}
          </div>
          <div className="text-[10px] text-amber-700/80 font-bold">Aguardando pagamento ou confirmação</div>
        </div>
      </div>

      {/* MOEDAS ACEITAS — configuração real, separada da documentação */}
      <div className="rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-xs space-y-3">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 border-b border-border/80 pb-3">
          <div className="flex items-center gap-2">
            <i className="bi bi-coin text-bitcoin text-base" />
            <h2 className="text-sm sm:text-base font-black text-ink">Moedas que você aceita receber</h2>
          </div>
          {saveSettingsMut.isPending && <span className="text-[11px] text-ink-muted">salvando…</span>}
          {saveSettingsMut.isError && (
            <span className="text-[11px] font-bold text-rose-600">não foi possível salvar</span>
          )}
        </div>

        <div className="flex flex-wrap gap-1.5">
          {availableCoins.map((c) => {
            const on = acceptedCoins.includes(c);
            return (
              <button
                key={c}
                type="button"
                data-testid={`accepted-coin-${c}`}
                disabled={saveSettingsMut.isPending}
                onClick={() => toggleAccepted(c)}
                className={clsx(
                  'flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-bold transition-all border',
                  on
                    ? 'bg-emerald-600 text-white border-emerald-600 shadow-xs'
                    : 'bg-surface text-ink-muted border-border hover:text-ink hover:bg-paper',
                )}
              >
                <img src={coinLogo(c)} alt={c} className="h-3.5 w-3.5 rounded-full object-contain" />
                <span>{c}</span>
                {on && <i className="bi bi-check2 font-bold text-xs" />}
              </button>
            );
          })}
        </div>
        <p className="text-[11px] text-ink-muted leading-relaxed">
          Quando você cobra em dólar, é esta lista que o cliente vê para escolher como pagar. Moedas com depósito
          pausado não aparecem aqui e nunca são oferecidas.
        </p>
      </div>

      {/* BOTÃO DE PAGAMENTO — o botão de verdade, não uma imitação */}
      <div className="rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-xs space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 border-b border-border/80 pb-3">
          <div className="flex items-center gap-2">
            <i className="bi bi-credit-card-2-front-fill text-bitcoin text-base" />
            <h2 className="text-sm sm:text-base font-black text-ink">Botão de pagamento</h2>
          </div>
          <Link to="/docs?tab=deposits" className="text-[11px] font-bold text-bitcoin hover:underline self-start sm:self-auto">
            Ver a documentação da API →
          </Link>
        </div>

        <p className="text-xs text-ink-muted leading-relaxed">
          Seu backend cria a fatura e recebe o <code className="font-mono text-ink font-bold">checkoutUrl</code>; o botão
          só leva o cliente até lá. Nenhuma chave de API vai para o navegador e o valor não pode ser adulterado no
          DevTools.
        </p>

        {/* Renderizado pelo próprio SDK: é exatamente o que o cliente vê. */}
        <div className="rounded-2xl border border-border bg-surface p-5">
          <div className="text-[10px] uppercase font-bold tracking-wider text-ink-muted mb-3">
            Como fica no seu site
          </div>
          <div ref={buttonPreviewRef} className="flex flex-wrap items-center gap-3">
            <div
              className="satspay-pay"
              data-checkout_url="/pay/demo"
              data-theme="bitcoin"
              data-size="large"
              data-label="Pagar com cripto"
              data-target="blank"
            />
            <div
              className="satspay-pay"
              data-checkout_url="/pay/demo"
              data-theme="light"
              data-size="large"
              data-label="Pagar com cripto"
              data-target="blank"
            />
            <div
              className="satspay-pay"
              data-checkout_url="/pay/demo"
              data-theme="dark"
              data-size="medium"
              data-shape="pill"
              data-label="Pagar"
              data-target="blank"
            />
          </div>
          <p className="mt-3 text-[11px] text-ink-muted">
            Botões reais, renderizados pelo SDK. Clicar abre o checkout de demonstração.
          </p>
        </div>

        <CodeBlock
          label="HTML"
          code={`<script src="${ORIGIN}/sdk/satspay-pay.js" async defer></script>

<div class="satspay-pay"
     data-checkout_url="COLE_O_checkoutUrl_DA_FATURA"
     data-theme="bitcoin"     <!-- bitcoin | light | dark | outline -->
     data-size="large"        <!-- small | medium | large -->
     data-shape="rounded"     <!-- rounded | pill | square -->
     data-label="Pagar com cripto"></div>`}
        />
      </div>

      {/* WEBHOOK TEST RESULT BANNER */}
      {testResult && (
        <div className="rounded-2xl border border-border bg-surface p-4 text-xs font-semibold text-ink shadow-xs animate-fade-in flex items-center gap-2">
          <span>{testResult.msg}</span>
        </div>
      )}

      {/* INVOICES TABLE & FILTER BAR */}
      <div className="rounded-3xl border border-border bg-paper overflow-hidden shadow-xs">
        <div className="p-4 sm:p-5 border-b border-border flex flex-col md:flex-row md:items-center md:justify-between gap-3 bg-surface/40">
          <div>
            <h2 className="text-sm font-bold text-ink flex items-center gap-2">
              <i className="bi bi-receipt-cutoff text-emerald-600" />
              <span>Extrato de Faturas do Gateway</span>
            </h2>
            <p className="text-[11px] text-ink-muted mt-0.5">
              Transações e depósitos processados pelo seu endpoint de cobrança.
            </p>
          </div>

          {/* Filters */}
          <div className="flex items-center gap-2 flex-wrap">
            {/* Status Filter */}
            <div className="flex items-center gap-1 rounded-xl bg-surface p-1 border border-border text-xs">
              {(['ALL', 'PAID', 'PENDING', 'EXPIRED'] as const).map((st) => (
                <button
                  key={st}
                  type="button"
                  onClick={() => setFilterStatus(st)}
                  className={clsx(
                    'rounded-lg px-2.5 py-1 font-bold text-[10px] transition-all',
                    filterStatus === st
                      ? 'bg-paper text-ink shadow-xs'
                      : 'text-ink-muted hover:text-ink',
                  )}
                >
                  {st === 'ALL' ? 'Todos' : st === 'PAID' ? 'Pagos' : st === 'PENDING' ? 'Pendentes' : 'Expirados'}
                </button>
              ))}
            </div>

            {/* Coin Filter */}
            <select
              value={filterCoin}
              onChange={(e) => setFilterCoin(e.target.value)}
              className="rounded-xl border border-border bg-surface px-2.5 py-1 text-xs font-bold text-ink"
            >
              <option value="ALL">Todas as Moedas</option>
              {COINS.map((c) => (
                <option key={c} value={c}>{c}</option>
              ))}
            </select>
          </div>
        </div>

        {isLoading ? (
          <div className="p-12 text-center text-ink-muted">
            <span className="h-6 w-6 animate-spin rounded-full border-2 border-emerald-600 border-t-transparent inline-block mb-2" />
            <div className="text-xs font-semibold">Carregando faturas da API...</div>
          </div>
        ) : filteredInvoices.length === 0 ? (
          <div className="p-12 text-center text-ink-muted space-y-3">
            <div className="flex h-12 w-12 mx-auto items-center justify-center rounded-full bg-surface text-ink-muted text-xl">
              <i className="bi bi-inbox" />
            </div>
            <div className="text-sm font-bold text-ink">Nenhuma fatura encontrada</div>
            <p className="text-xs text-ink-muted max-w-sm mx-auto">
              Quando sua aplicação fizer chamadas para o endpoint de criação de depósitos, as cobranças aparecerão em tempo real nesta lista.
            </p>
            <Link
              to="/docs"
              className="rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white font-bold text-xs px-4 py-2 shadow-xs transition-all inline-flex items-center gap-1.5"
            >
              <i className="bi bi-code-slash" />
              <span>Ver Como Integrar na Documentação</span>
            </Link>
          </div>
        ) : (
          <div className="overflow-x-auto [scrollbar-width:none]">
            <table className="w-full text-left text-xs">
              <thead className="bg-surface text-[10px] uppercase font-bold text-ink-muted border-b border-border">
                <tr>
                  <th className="p-3.5">Ordem / Identificador</th>
                  <th className="p-3.5">Cliente / Site</th>
                  <th className="p-3.5">Moeda</th>
                  <th className="p-3.5">Valor Cobrado</th>
                  <th className="p-3.5">Status</th>
                  <th className="p-3.5">Webhook IPN</th>
                  <th className="p-3.5">Data / Hora</th>
                  <th className="p-3.5">Checkout</th>
                  <th className="p-3.5 text-right">Ação</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border font-medium">
                {filteredInvoices.map((inv) => {
                  const isPaid = inv.status === 'CONFIRMED' || inv.status === 'PAID';
                  const isPending = inv.status === 'PENDING' || inv.status === 'DETECTED';

                  return (
                    <tr key={inv.id} className="hover:bg-surface/50 transition-colors">
                      <td className="p-3.5">
                        <div className="font-mono font-bold text-ink">{inv.orderId}</div>
                        <div className="text-[10px] text-ink-muted font-mono truncate max-w-[120px]">
                          {inv.id}
                        </div>
                      </td>

                      <td className="p-3.5">
                        <div className="font-bold text-ink truncate max-w-[160px]">
                          {inv.customerEmail || 'Cliente Anônimo'}
                        </div>
                        <div className="text-[10px] text-ink-muted truncate max-w-[160px]">
                          {inv.siteName || inv.siteUserId || 'Site Parceiro'}
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

                      <td className="p-3.5 font-mono font-bold text-ink">
                        {formatLedgerAmount(inv.amount, inv.coin)}{' '}
                        <span className="text-[10px] text-ink-muted">{inv.coin}</span>
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
                              <i className="bi bi-check-circle-fill" /> PAGO
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

                      <td className="p-3.5">
                        {inv.webhookDelivered ? (
                          <span className="inline-flex items-center gap-1 text-emerald-600 font-bold text-[10px]">
                            <i className="bi bi-check2-all text-xs" />
                            <span>Entregue ({inv.webhookStatusCode || 200})</span>
                          </span>
                        ) : inv.webhookAttempts > 0 ? (
                          <span className="inline-flex items-center gap-1 text-rose-600 font-bold text-[10px]">
                            <i className="bi bi-exclamation-triangle-fill text-xs" />
                            <span>Falha ({inv.webhookAttempts}x)</span>
                          </span>
                        ) : isPaid ? (
                          <span className="text-ink-muted text-[10px]">Aguardando envio</span>
                        ) : (
                          // A webhook only ever fires for a confirmed invoice.
                          // Saying "pending" on an expired one promised a
                          // delivery that was never going to happen.
                          <span className="text-ink-muted/60 text-[10px]" title="Webhook só é enviado quando a fatura é confirmada">
                            —
                          </span>
                        )}
                      </td>

                      <td className="p-3.5 font-mono text-ink-muted text-[11px]">
                        {new Date(inv.createdAt).toLocaleString('pt-BR', {
                          day: '2-digit',
                          month: '2-digit',
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </td>

                      <td className="p-3.5">
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

                      <td className="p-3.5 text-right">
                        <button
                          type="button"
                          onClick={() => testWebhookMut.mutate(inv.id)}
                          disabled={testWebhookMut.isPending}
                          className="rounded-lg bg-surface border border-border px-2 py-1 text-[10px] font-bold text-ink-muted hover:text-ink hover:bg-paper active:scale-95 shadow-xs"
                          title="Disparar webhook de teste para o callbackUrl"
                        >
                          Testar Webhook
                        </button>
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

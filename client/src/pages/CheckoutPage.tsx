import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { coinLogo } from '../lib/coinAssets.js';
import { addressQrDataUrl } from '../lib/qr.js';
import { useAuthStore } from '../stores/auth.js';
import { formatAmount, safeBigInt, type Coin } from '@/shared';

interface InvoiceData {
  id: string;
  status: 'PENDING' | 'DETECTED' | 'CONFIRMED' | 'EXPIRED' | 'CANCELLED';
  coin: Coin;
  amount: string;
  depositAddress: string;
  orderId: string;
  siteName?: string;
  description?: string;
  customerEmail?: string;
  successUrl?: string;
  cancelUrl?: string;
  qrCode: string;
  expiresAt: string;
  paidAt?: string;
  txHash?: string;
  /** `amount` rendered as a quantity of coins, e.g. "25" for 25 USDT. */
  amountDisplay?: string;
  /** Set by `/pay/demo`: nothing here is real and nothing can be paid. */
  demo?: boolean;
}

/**
 * The host actually serving this page. Hardcoding it meant the checkout —
 * the one page a customer inspects before sending money — advertised
 * "satspay.app", a domain this platform does not use.
 */
function checkoutHost(): string {
  if (typeof window === 'undefined' || !window.location?.host) return 'satspay.pro';
  return window.location.host.replace(/^www\./, '');
}

interface CoinCatalog {
  priceDecimals: number;
  coins: { symbol: string; priceUsd?: string | null }[];
}

/**
 * Approximate fiat value of a ledger amount, or null when the coin has no
 * cached price. The checkout showed no fiat at all, so a customer was asked
 * for "0.00001 LTC" with no idea what that costs.
 *
 * Total by construction: this renders on the page a customer pays on, so a
 * missing, malformed or partial catalogue must degrade to "no label", never
 * to a blank checkout.
 */
function fiatValue(amountUnits: string, coin: Coin, catalog?: CoinCatalog): string | null {
  try {
    const list = Array.isArray(catalog?.coins) ? catalog.coins : [];
    const entry = list.find((c) => c?.symbol === coin);
    if (!entry?.priceUsd) return null;

    const decimals = Number(catalog?.priceDecimals);
    const price = Number(entry.priceUsd) / 10 ** (Number.isFinite(decimals) ? decimals : 8);
    const coins = Number(formatAmount(safeBigInt(amountUnits), coin));
    if (!Number.isFinite(price) || !Number.isFinite(coins) || price <= 0) return null;

    return (coins * price).toLocaleString('pt-BR', {
      style: 'currency',
      currency: 'USD',
      maximumFractionDigits: 2,
    });
  } catch {
    return null;
  }
}

export function CheckoutPage() {
  const { id } = useParams<{ id: string }>();
  const user = useAuthStore((s) => s.user);
  const [copied, setCopied] = useState(false);
  const [timeLeft, setTimeLeft] = useState<string>('');
  const [redirectCount, setRedirectCount] = useState<number>(4);
  const [qrUrl, setQrUrl] = useState<string>('');

  const { data: inv, isLoading, error } = useQuery<InvoiceData>({
    queryKey: ['public-invoice', id],
    queryFn: () => api<InvoiceData>(`/public/pay/${id}`),
    refetchInterval: (query) => {
      const st = query.state.data?.status;
      if (st === 'CONFIRMED' || st === 'EXPIRED' || st === 'CANCELLED') {
        return false;
      }
      return 3000;
    },
    enabled: !!id,
  });

  const payBalanceMut = useMutation({
    mutationFn: () => api(`/public/pay/${id}/balance`, { method: 'POST' }),
  });

  // Public catalogue: price for the fiat label. A failure here must never
  // block the payment UI, so the label simply does not render.
  const { data: catalog } = useQuery<CoinCatalog>({
    queryKey: ['public-coins'],
    queryFn: () => api<CoinCatalog>('/public/coins'),
    staleTime: 60_000,
    retry: false,
  });

  // Generate QR Code data URL
  useEffect(() => {
    if (!inv?.depositAddress) return;
    const qrData = inv.qrCode || inv.depositAddress;
    addressQrDataUrl(qrData)
      .then(setQrUrl)
      .catch(() => {});
  }, [inv?.depositAddress, inv?.qrCode]);

  // Countdown timer calculation
  useEffect(() => {
    if (!inv?.expiresAt || inv.status === 'CONFIRMED') return;
    const interval = setInterval(() => {
      const diff = new Date(inv.expiresAt).getTime() - Date.now();
      if (diff <= 0) {
        setTimeLeft('00:00');
        clearInterval(interval);
      } else {
        const mins = Math.floor((diff / 1000 / 60) % 60);
        const secs = Math.floor((diff / 1000) % 60);
        setTimeLeft(`${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`);
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [inv?.expiresAt, inv?.status]);

  // Automatic redirect upon confirmation
  useEffect(() => {
    if (inv?.status === 'CONFIRMED' && inv.successUrl) {
      const t = setInterval(() => {
        setRedirectCount((c) => {
          if (c <= 1) {
            clearInterval(t);
            window.location.href = inv.successUrl!;
            return 0;
          }
          return c - 1;
        });
      }, 1000);
      return () => clearInterval(t);
    }
  }, [inv?.status, inv?.successUrl]);

  const copyAddress = () => {
    if (!inv?.depositAddress) return;
    navigator.clipboard.writeText(inv.depositAddress);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas text-ink p-4">
        <div className="flex flex-col items-center gap-3">
          <div className="h-10 w-10 animate-spin rounded-full border-4 border-bitcoin border-t-transparent" />
          <span className="text-sm font-semibold text-ink-muted">Carregando checkout seguro...</span>
        </div>
      </div>
    );
  }

  if (error || !inv) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas text-ink p-4">
        <div className="max-w-md w-full rounded-2xl border border-border bg-paper p-8 text-center shadow-card">
          <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-rose-500/10 text-rose-600 text-2xl">
            <i className="bi bi-x-circle-fill" />
          </div>
          <h1 className="text-xl font-bold">Cobrança Não Encontrada</h1>
          <p className="mt-2 text-sm text-ink-muted">
            Este link de pagamento não existe ou expirou. Por favor, volte ao site de origem e tente novamente.
          </p>
          <Link
            to="/"
            className="mt-6 inline-block rounded-xl bg-surface px-5 py-2.5 text-sm font-semibold text-ink border border-border hover:bg-paper"
          >
            Ir para SatsPay
          </Link>
        </div>
      </div>
    );
  }

  const isConfirmed = inv.status === 'CONFIRMED';
  const isExpired = inv.status === 'EXPIRED';

  return (
    <div className="min-h-screen bg-canvas text-ink flex flex-col justify-between selection:bg-bitcoin/30">
      {/* HEADER */}
      <header className="border-b border-border/80 bg-paper/80 backdrop-blur-md sticky top-0 z-10">
        <div className="mx-auto flex h-16 max-w-2xl items-center justify-between px-4">
          <div className="flex items-center gap-2.5">
            <img src="/logo.png" alt="SatsPay" className="h-8 w-8 object-contain" />
            <span className="font-extrabold tracking-tight text-lg text-ink">SatsPay</span>
            <span className="text-[10px] font-bold uppercase tracking-wider bg-emerald-500/10 text-emerald-600 border border-emerald-500/20 rounded-full px-2 py-0.5">
              Checkout Seguro
            </span>
          </div>
          {inv.siteName && (
            <div className="text-xs font-semibold text-ink-muted flex items-center gap-1.5">
              <i className="bi bi-shop text-bitcoin" />
              <span>{inv.siteName}</span>
            </div>
          )}
        </div>
      </header>

      {/* MAIN CHECKOUT CONTAINER */}
      <main className="flex-1 flex items-center justify-center p-4 py-8">
        <div className="w-full max-w-md rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-2xl relative overflow-hidden">
          {inv.demo && (
            <div className="mb-4 rounded-2xl border border-amber-400/40 bg-amber-50 px-4 py-3 text-xs text-amber-900">
              <div className="flex items-center gap-2 font-bold">
                <i className="bi bi-eye-fill" />
                <span>Demonstração</span>
              </div>
              <p className="mt-1 leading-relaxed">
                Esta é uma fatura de exemplo para você ver o checkout. O endereço não é real,
                nenhum pagamento é processado e nenhum webhook é disparado.
              </p>
            </div>
          )}
          {/* SUCCESS STATE */}
          {isConfirmed ? (
            <div className="text-center py-6 space-y-4 animate-scale-up">
              <div className="mx-auto flex h-20 w-20 items-center justify-center rounded-3xl bg-emerald-500/15 text-emerald-600 text-4xl shadow-inner ring-4 ring-emerald-500/10">
                <i className="bi bi-check-lg font-extrabold" />
              </div>
              <h2 className="text-2xl font-black text-ink tracking-tight">Depósito Confirmado!</h2>
              <p className="text-sm text-ink-muted leading-relaxed">
                Seu pagamento de <b className="text-ink font-mono">{inv.amount} {inv.coin}</b> foi processado e creditado com sucesso.
              </p>

              <div className="rounded-2xl border border-border bg-surface p-4 text-xs space-y-2 text-left">
                <div className="flex justify-between">
                  <span className="text-ink-muted">Pedido / Ref:</span>
                  <span className="font-mono font-bold text-ink">{inv.orderId}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-ink-muted">Status:</span>
                  <span className="font-bold text-emerald-600">PAGO & LIQUIDADO</span>
                </div>
                {inv.txHash && (
                  <div className="flex justify-between truncate">
                    <span className="text-ink-muted">Hash Tx:</span>
                    <span className="font-mono text-[11px] text-bitcoin-dark truncate max-w-[180px]">{inv.txHash}</span>
                  </div>
                )}
              </div>

              {inv.successUrl ? (
                <div className="pt-2">
                  <a
                    href={inv.successUrl}
                    className="block w-full rounded-2xl bg-bitcoin py-3.5 text-center font-bold text-white shadow-lg shadow-bitcoin/25 hover:bg-bitcoin-dark transition-transform hover:scale-[1.02]"
                  >
                    Voltar para {inv.siteName || 'a Loja'} ({redirectCount}s)
                  </a>
                </div>
              ) : (
                <div className="pt-2">
                  <Link
                    to="/"
                    className="block w-full rounded-2xl bg-surface border border-border py-3 text-center text-sm font-bold text-ink hover:bg-paper"
                  >
                    Concluído
                  </Link>
                </div>
              )}
            </div>
          ) : isExpired ? (
            /* EXPIRED STATE */
            <div className="text-center py-6 space-y-4">
              <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-rose-500/15 text-rose-600 text-3xl">
                <i className="bi bi-clock-history" />
              </div>
              <h2 className="text-xl font-bold text-ink">Tempo Esgotado</h2>
              <p className="text-xs text-ink-muted">
                O prazo de 60 minutos para este depósito expirou. Por favor, solicite um novo depósito no site de origem.
              </p>
              {inv.cancelUrl && (
                <a
                  href={inv.cancelUrl}
                  className="inline-block mt-2 rounded-xl bg-surface border border-border px-5 py-2.5 text-xs font-bold text-ink hover:bg-paper"
                >
                  Voltar para {inv.siteName || 'o site'}
                </a>
              )}
            </div>
          ) : (
            /* ACTIVE PAYMENT STATE */
            <div className="space-y-5">
              {/* Order Info Bar */}
              <div className="flex items-center justify-between border-b border-border/70 pb-4">
                <div>
                  <div className="text-[11px] uppercase font-bold tracking-wider text-ink-muted">
                    Depósito Solicitado
                  </div>
                  <div className="text-xs font-semibold text-ink mt-0.5">
                    Pedido: <span className="font-mono">{inv.orderId}</span>
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-[10px] uppercase font-bold tracking-wider text-ink-muted flex items-center justify-end gap-1">
                    <i className="bi bi-stopwatch text-bitcoin" />
                    <span>Expira em</span>
                  </div>
                  <div className="text-xs font-mono font-bold text-bitcoin-dark mt-0.5">{timeLeft || '60:00'}</div>
                </div>
              </div>

              {/* Amount Box */}
              <div className="rounded-2xl border border-border bg-surface/70 p-4 text-center">
                <div className="flex items-center justify-center gap-2 mb-1">
                  <img src={coinLogo(inv.coin)} alt={inv.coin} className="h-6 w-6 rounded-full object-contain" />
                  <span className="text-xs font-bold text-ink-muted">{inv.coin}</span>
                </div>
                <div className="font-mono text-3xl font-black tracking-tight text-ink">
                  {inv.amountDisplay ?? formatAmount(safeBigInt(inv.amount), inv.coin)}{' '}
                  <span className="text-lg font-bold text-ink-muted">{inv.coin}</span>
                </div>
                {fiatValue(inv.amount, inv.coin, catalog) && (
                  <div className="text-xs font-semibold text-ink-muted mt-1">
                    ≈ {fiatValue(inv.amount, inv.coin, catalog)}{' '}
                    <span className="font-normal">(cotação estimada)</span>
                  </div>
                )}
                {inv.description && (
                  <div className="text-[11px] text-ink-muted mt-1">{inv.description}</div>
                )}
              </div>

              {/* QR Code */}
              <div className="flex flex-col items-center justify-center py-2">
                <div className="flex h-48 w-48 items-center justify-center rounded-2xl border border-border bg-white p-3 shadow-md">
                  {qrUrl ? (
                    <img
                      src={qrUrl}
                      alt="QR Code de Pagamento"
                      className="h-full w-full object-contain"
                    />
                  ) : (
                    <div className="h-6 w-6 animate-spin rounded-full border-2 border-bitcoin border-t-transparent" />
                  )}
                </div>
                <span className="text-[10px] font-medium text-ink-muted mt-2">
                  Escaneie com sua carteira ou exchange
                </span>
              </div>

              {/* Deposit Address Box */}
              <div>
                <label className="block text-[10px] uppercase font-bold tracking-wider text-ink-muted mb-1.5 flex items-center justify-between">
                  <span>Endereço de Destino</span>
                  <span className="text-emerald-600 font-semibold lowercase">rede oficial</span>
                </label>
                <div className="flex items-center gap-2 rounded-2xl border border-border bg-surface p-2.5">
                  <div className="truncate font-mono text-xs text-ink flex-1 px-1 select-all">
                    {inv.depositAddress}
                  </div>
                  <button
                    type="button"
                    onClick={copyAddress}
                    className="shrink-0 rounded-xl bg-paper px-3 py-1.5 text-xs font-bold text-bitcoin-dark border border-border hover:bg-surface transition-all active:scale-95"
                  >
                    {copied ? (
                      <span className="text-emerald-600 flex items-center gap-1">
                        <i className="bi bi-check" /> Copiado
                      </span>
                    ) : (
                      <span className="flex items-center gap-1">
                        <i className="bi bi-copy" /> Copiar
                      </span>
                    )}
                  </button>
                </div>
              </div>

              {/* Pay with Internal SatsPay Balance */}
              {user && !inv.demo && (
                <div className="pt-2 border-t border-border/70">
                  <button
                    type="button"
                    disabled={payBalanceMut.isPending}
                    onClick={() => payBalanceMut.mutate()}
                    className="w-full rounded-2xl bg-gradient-to-r from-bitcoin to-bitcoin-dark py-3 px-4 text-xs font-bold text-white shadow-md shadow-bitcoin/25 hover:opacity-95 transition-all flex items-center justify-center gap-2"
                  >
                    <i className="bi bi-lightning-charge-fill" />
                    <span>
                      {payBalanceMut.isPending
                        ? 'Processando pagamento...'
                        : 'Pagar com 1-Clique (Saldo SatsPay)'}
                    </span>
                  </button>
                  {payBalanceMut.isError && (
                    <div className="mt-2 text-center text-[11px] text-rose-600 font-medium">
                      Saldo insuficiente ou erro ao processar. Use o endereço acima.
                    </div>
                  )}
                </div>
              )}

              {/* Detection status spinner */}
              <div className="flex items-center justify-center gap-2 text-xs text-ink-muted pt-1">
                <div className="h-3 w-3 animate-spin rounded-full border-2 border-bitcoin border-t-transparent" />
                <span>Aguardando transferência na blockchain...</span>
              </div>
            </div>
          )}
        </div>
      </main>

      {/* FOOTER */}
      <footer className="border-t border-border/70 py-4 text-center text-xs text-ink-muted bg-paper/50">
        <div className="mx-auto max-w-2xl px-4 flex items-center justify-between">
          <div className="flex items-center gap-1">
            <i className="bi bi-shield-lock-fill text-emerald-600" />
            <span>Processado com segurança por <b>SatsPay</b></span>
          </div>
          <Link to="/" className="text-ink-muted hover:text-ink">
            {checkoutHost()}
          </Link>
        </div>
      </footer>
    </div>
  );
}

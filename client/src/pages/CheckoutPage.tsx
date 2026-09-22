import { useEffect, useMemo, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { api, ApiError } from '../lib/api.js';
import { coinLogo } from '../lib/coinAssets.js';
import { addressQrDataUrl } from '../lib/qr.js';
import { useAuthStore } from '../stores/auth.js';
import { LanguageSwitch } from '../components/LanguageSwitch.js';
import { formatAmount, safeBigInt, type Coin, type WalletBalance } from '@/shared';

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
  amountDisplay?: string;
  demo?: boolean;
  coinOptions?: CoinOption[];
  coinLocked?: boolean;
  amountUsd?: string | null;
}

interface CoinOption {
  coin: Coin;
  name: string;
  amount: string;
  amountDisplay: string;
  logoUrl: string;
  minConfirmations: number;
}

interface CoinCatalog {
  priceDecimals: number;
  coins: { symbol: string; priceUsd?: string | null }[];
}

function checkoutHost(): string {
  if (typeof window === 'undefined' || !window.location?.host) return 'satspay.pro';
  return window.location.host.replace(/^www\./, '');
}

function asWallets(data: unknown): WalletBalance[] {
  if (Array.isArray(data)) return data as WalletBalance[];
  if (data && typeof data === 'object' && Array.isArray((data as { wallets?: unknown }).wallets)) {
    return (data as { wallets: WalletBalance[] }).wallets;
  }
  return [];
}

function coinNetwork(coin: Coin): string {
  if (coin === 'SOL') return 'Solana';
  if (coin === 'POL' || coin === 'USDT' || coin === 'USDC') return 'Polygon';
  if (coin === 'PEPE') return 'BNB Smart Chain (BEP-20)';
  if (coin === 'ZER') return 'Zero';
  return coin;
}

function fiatValue(amountUnits: string, coin: Coin, catalog?: CoinCatalog, locale = 'pt-BR'): string | null {
  try {
    const list = Array.isArray(catalog?.coins) ? catalog.coins : [];
    const entry = list.find((c) => c?.symbol === coin);
    if (!entry?.priceUsd) return null;
    const decimals = Number(catalog?.priceDecimals);
    const price = Number(entry.priceUsd) / 10 ** (Number.isFinite(decimals) ? decimals : 8);
    const coins = Number(formatAmount(safeBigInt(amountUnits), coin));
    if (!Number.isFinite(price) || !Number.isFinite(coins) || price <= 0) return null;
    return (coins * price).toLocaleString(locale, {
      style: 'currency',
      currency: 'USD',
      maximumFractionDigits: 2,
    });
  } catch {
    return null;
  }
}

function displayAmount(inv: InvoiceData): string {
  return inv.amountDisplay ?? formatAmount(safeBigInt(inv.amount), inv.coin);
}

export function CheckoutPage() {
  const { id } = useParams<{ id: string }>();
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();
  const user = useAuthStore((s) => s.user);
  const [copied, setCopied] = useState(false);
  const [timeLeft, setTimeLeft] = useState<string>('');
  const [redirectCount, setRedirectCount] = useState<number>(4);
  const [qrUrl, setQrUrl] = useState<string>('');
  const [coinChosen, setCoinChosen] = useState(false);
  const locale = i18n.resolvedLanguage?.startsWith('en') ? 'en-US' : 'pt-BR';

  const { data: inv, isLoading, error } = useQuery<InvoiceData>({
    queryKey: ['public-invoice', id],
    queryFn: () => api<InvoiceData>(`/public/pay/${id}`),
    refetchInterval: (query) => {
      const st = query.state.data?.status;
      if (st === 'CONFIRMED' || st === 'EXPIRED' || st === 'CANCELLED') return false;
      return 3000;
    },
    enabled: !!id,
  });

  const payBalanceMut = useMutation({
    mutationFn: () => api(`/public/pay/${id}/balance`, { method: 'POST' }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['public-invoice', id] });
      void qc.invalidateQueries({ queryKey: ['wallets'] });
    },
  });

  const selectCoinMut = useMutation({
    mutationFn: (coin: Coin) =>
      api(`/public/pay/${id}/select-coin`, { method: 'POST', json: { coin } }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['public-invoice', id] });
      setCoinChosen(true);
    },
  });

  const { data: catalog } = useQuery<CoinCatalog>({
    queryKey: ['public-coins'],
    queryFn: () => api<CoinCatalog>('/public/coins'),
    staleTime: 60_000,
    retry: false,
  });

  const walletsQ = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<WalletBalance[] | { wallets: WalletBalance[] }>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user) && Boolean(inv) && !inv?.demo,
    staleTime: 15_000,
  });

  const pickerCoins = useMemo((): CoinOption[] => {
    const offered = inv?.coinOptions ?? [];
    const stables = ['USDT', 'USDC'];
    return [...offered].sort((a, b) => {
      const as = stables.includes(a.coin) ? 0 : 1;
      const bs = stables.includes(b.coin) ? 0 : 1;
      return as - bs;
    });
  }, [inv]);

  const walletUnits = (coin: Coin): bigint => {
    const row = asWallets(walletsQ.data).find((w) => w.coin === coin);
    return safeBigInt(row?.balance);
  };

  useEffect(() => {
    if (!inv?.depositAddress) return;
    addressQrDataUrl(inv.qrCode || inv.depositAddress)
      .then(setQrUrl)
      .catch(() => {});
  }, [inv?.depositAddress, inv?.qrCode]);

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

  useEffect(() => {
    if (inv?.status === 'CONFIRMED' && inv.successUrl) {
      const tmr = setInterval(() => {
        setRedirectCount((c) => {
          if (c <= 1) {
            clearInterval(tmr);
            window.location.href = inv.successUrl!;
            return 0;
          }
          return c - 1;
        });
      }, 1000);
      return () => clearInterval(tmr);
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
          <span className="text-sm font-semibold text-ink-muted">{t('checkout.loading')}</span>
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
          <h1 className="text-xl font-bold">{t('checkout.notFound.title')}</h1>
          <p className="mt-2 text-sm text-ink-muted">{t('checkout.notFound.body')}</p>
          <Link
            to="/"
            className="mt-6 inline-block rounded-xl bg-surface px-5 py-2.5 text-sm font-semibold text-ink border border-border hover:bg-paper"
          >
            {t('checkout.notFound.home')}
          </Link>
        </div>
      </div>
    );
  }

  const isConfirmed = inv.status === 'CONFIRMED';
  const isExpired = inv.status === 'EXPIRED' || inv.status === 'CANCELLED';
  const paidWithBalance = inv.txHash === 'internal_satspay';
  const canSwitchCoin = !inv.coinLocked && (inv.coinOptions?.length ?? 0) > 1;
  const picking = canSwitchCoin && !coinChosen;
  const fiat = fiatValue(inv.amount, inv.coin, catalog, locale);
  const loginTo = `/login?return_to=${encodeURIComponent(`/pay/${id}`)}`;

  return (
    <div className="min-h-screen bg-canvas text-ink flex flex-col justify-between selection:bg-bitcoin/30">
      <header className="border-b border-border/80 bg-paper/80 backdrop-blur-md sticky top-0 z-10">
        <div className="mx-auto flex h-16 max-w-2xl items-center justify-between gap-3 px-4">
          <div className="flex items-center gap-2.5 min-w-0">
            <img src="/logo.png" alt="SatsPay" className="h-8 w-8 object-contain" />
            <span className="font-extrabold tracking-tight text-lg text-ink">SatsPay</span>
            <span className="hidden sm:inline text-[10px] font-bold uppercase tracking-wider bg-emerald-500/10 text-emerald-600 border border-emerald-500/20 rounded-full px-2 py-0.5">
              {t('checkout.badge')}
            </span>
          </div>
          <div className="flex items-center gap-2.5 shrink-0">
            {inv.siteName && (
              <div className="text-xs font-semibold text-ink-muted flex items-center gap-1.5 max-w-[10rem] truncate">
                <i className="bi bi-shop text-bitcoin" />
                <span className="truncate">{inv.siteName}</span>
              </div>
            )}
            <LanguageSwitch />
          </div>
        </div>
      </header>

      <main className="flex-1 flex items-center justify-center p-4 py-8">
        <div className="w-full max-w-md rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-2xl relative overflow-hidden">
          {inv.demo && (
            <div className="mb-5 rounded-2xl border border-amber-400/40 bg-amber-50 px-4 py-3 text-xs text-amber-900">
              <div className="flex items-center gap-2 font-bold">
                <i className="bi bi-eye-fill" />
                <span>{t('checkout.demo.title')}</span>
              </div>
              <p className="mt-1 leading-relaxed">{t('checkout.demo.body')}</p>
            </div>
          )}

          {isConfirmed ? (
            <div className="text-center py-4 space-y-5 animate-scale-up">
              <div className="mx-auto flex h-20 w-20 items-center justify-center rounded-full bg-emerald-500 text-white text-4xl shadow-lg shadow-emerald-500/30">
                <i className="bi bi-check-lg" />
              </div>
              <div className="space-y-1">
                <h2 className="text-2xl font-black text-ink tracking-tight">{t('checkout.success.title')}</h2>
                <p className="text-sm text-ink-muted leading-relaxed">
                  {t('checkout.success.body', {
                    amount: displayAmount(inv),
                    coin: inv.coin,
                    store: inv.siteName || t('checkout.theStore'),
                  })}
                </p>
              </div>
              <div className="rounded-2xl border border-border bg-surface p-4 text-xs space-y-2.5 text-left">
                <div className="flex justify-between gap-3">
                  <span className="text-ink-muted">{t('checkout.success.paid')}</span>
                  <span className="font-mono font-bold text-ink">
                    {displayAmount(inv)} {inv.coin}
                  </span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-ink-muted">{t('checkout.order')}</span>
                  <span className="font-mono font-bold text-ink">{inv.orderId}</span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-ink-muted">{t('checkout.success.method')}</span>
                  <span className="font-bold text-emerald-700">
                    {paidWithBalance ? t('checkout.success.methodBalance') : t('checkout.success.methodOnchain')}
                  </span>
                </div>
                {inv.txHash && !paidWithBalance && (
                  <div className="flex justify-between gap-3">
                    <span className="text-ink-muted shrink-0">{t('checkout.success.tx')}</span>
                    <span className="font-mono text-[11px] text-bitcoin-dark truncate max-w-[180px]">{inv.txHash}</span>
                  </div>
                )}
              </div>
              {inv.successUrl ? (
                <a
                  href={inv.successUrl}
                  className="block w-full rounded-2xl bg-bitcoin py-3.5 text-center font-bold text-white shadow-lg shadow-bitcoin/25 hover:bg-bitcoin-dark transition-transform hover:scale-[1.02]"
                >
                  {t('checkout.success.backStore', { store: inv.siteName || t('checkout.theStore'), seconds: redirectCount })}
                </a>
              ) : (
                <Link
                  to="/"
                  className="block w-full rounded-2xl bg-surface border border-border py-3 text-center text-sm font-bold text-ink hover:bg-paper"
                >
                  {t('checkout.success.done')}
                </Link>
              )}
            </div>
          ) : isExpired ? (
            <div className="text-center py-6 space-y-4">
              <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-rose-500/15 text-rose-600 text-3xl">
                <i className="bi bi-clock-history" />
              </div>
              <h2 className="text-xl font-bold text-ink">
                {inv.status === 'CANCELLED' ? t('checkout.cancelled.title') : t('checkout.expired.title')}
              </h2>
              <p className="text-sm text-ink-muted leading-relaxed">
                {inv.status === 'CANCELLED' ? t('checkout.cancelled.body') : t('checkout.expired.body')}
              </p>
              {inv.cancelUrl && (
                <a
                  href={inv.cancelUrl}
                  className="inline-block mt-2 rounded-xl bg-surface border border-border px-5 py-2.5 text-xs font-bold text-ink hover:bg-paper"
                >
                  {t('checkout.expired.back', { store: inv.siteName || t('checkout.theStore') })}
                </a>
              )}
            </div>
          ) : (
            <div className="space-y-5">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="text-[11px] uppercase font-bold tracking-wider text-ink-muted">{t('checkout.payTitle')}</div>
                  <div className="text-xs font-semibold text-ink mt-0.5 truncate">
                    {t('checkout.order')}: <span className="font-mono">{inv.orderId}</span>
                  </div>
                </div>
                <div className="text-right shrink-0">
                  <div className="text-[10px] uppercase font-bold tracking-wider text-ink-muted flex items-center justify-end gap-1">
                    <i className="bi bi-stopwatch text-bitcoin" />
                    <span>{t('checkout.expires')}</span>
                  </div>
                  <div className="text-sm font-mono font-bold text-bitcoin-dark mt-0.5">{timeLeft || '60:00'}</div>
                </div>
              </div>

              <div className="rounded-3xl bg-gradient-to-br from-surface to-paper border border-border px-4 py-5 text-center">
                <div className="text-[11px] uppercase font-bold tracking-wider text-ink-muted mb-1">{t('checkout.total')}</div>
                {inv.amountUsd ? (
                  <div className="font-black tracking-tight text-ink text-4xl">US$ {inv.amountUsd}</div>
                ) : (
                  <div className="font-mono font-black tracking-tight text-ink text-3xl">
                    {displayAmount(inv)} <span className="text-lg text-ink-muted">{inv.coin}</span>
                  </div>
                )}
                {inv.description && <p className="text-[11px] text-ink-muted mt-2">{inv.description}</p>}
              </div>

              {picking ? (
                <div className="rounded-2xl border border-border bg-surface/70 p-4 space-y-2">
                  <span className="text-xs font-bold text-ink">{t('checkout.crypto.choose')}</span>
                  <p className="text-[11px] text-ink-muted">{t('checkout.crypto.chooseHint')}</p>
                  <div className="space-y-1.5">
                    {pickerCoins.map((o) => (
                      <button
                        key={o.coin}
                        type="button"
                        disabled={selectCoinMut.isPending}
                        onClick={() => selectCoinMut.mutate(o.coin)}
                        className="flex w-full items-center justify-between gap-3 rounded-xl border border-border bg-paper px-3 py-2.5 text-left transition hover:border-bitcoin/50 hover:bg-bitcoin/[0.03]"
                      >
                        <span className="flex items-center gap-2 min-w-0">
                          <img src={o.logoUrl} alt="" className="h-7 w-7 rounded-full object-contain" />
                          <span className="min-w-0">
                            <span className="block text-xs font-bold text-ink">{o.name}</span>
                            <span className="block text-[10px] text-ink-muted truncate">
                              {o.coin} · {coinNetwork(o.coin)}
                            </span>
                          </span>
                        </span>
                        <span className="text-right shrink-0">
                          <span className="block font-mono text-sm font-bold text-ink">
                            {o.amountDisplay} {o.coin}
                          </span>
                          <span className="block text-[10px] text-ink-muted">
                            {t('checkout.crypto.confirms', { count: o.minConfirmations })}
                          </span>
                        </span>
                      </button>
                    ))}
                  </div>
                  <p className="text-[11px] text-ink-muted leading-relaxed">{t('checkout.crypto.quoteLocked')}</p>
                  {selectCoinMut.isError && (
                    <p className="text-[11px] font-bold text-rose-600">{t('checkout.crypto.switchError')}</p>
                  )}
                </div>
              ) : (
                <>
                  {canSwitchCoin && (
                    <div className="text-center">
                      <button type="button" onClick={() => setCoinChosen(false)} className="text-[11px] font-bold text-bitcoin hover:underline">
                        <i className="bi bi-arrow-left-right" /> {t('checkout.crypto.switch')}
                      </button>
                    </div>
                  )}

                  <div className="rounded-2xl border border-border bg-surface/70 p-4 text-center">
                    <div className="flex items-center justify-center gap-2 mb-1">
                      <img src={coinLogo(inv.coin)} alt={inv.coin} className="h-6 w-6 rounded-full object-contain" />
                      <span className="text-xs font-bold text-ink-muted">
                        {inv.coin} · {coinNetwork(inv.coin)}
                      </span>
                    </div>
                    <div className="font-mono text-3xl font-black tracking-tight text-ink">
                      {displayAmount(inv)} <span className="text-lg font-bold text-ink-muted">{inv.coin}</span>
                    </div>
                    {fiat && (
                      <div className="text-xs font-semibold text-ink-muted mt-1">
                        ≈ {fiat} <span className="font-normal">{t('checkout.crypto.estQuote')}</span>
                      </div>
                    )}
                  </div>

                  <section className="rounded-2xl border-2 border-bitcoin/30 bg-bitcoin/5 p-4 space-y-3">
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <div className="text-xs font-black text-ink">{t('checkout.balance.title')}</div>
                        <p className="text-[11px] text-ink-muted mt-0.5">{t('checkout.balance.instant')}</p>
                      </div>
                      <span className="shrink-0 rounded-full bg-bitcoin/10 text-bitcoin-dark text-[10px] font-bold uppercase tracking-wider px-2 py-0.5">
                        {t('checkout.balance.badge')}
                      </span>
                    </div>
                    {inv.demo ? (
                      <button
                        type="button"
                        disabled
                        className="w-full rounded-2xl bg-bitcoin/40 py-3 px-4 text-xs font-bold text-white cursor-not-allowed"
                      >
                        <i className="bi bi-lightning-charge-fill mr-1.5" />
                        {t('checkout.balance.payCoin', { amount: displayAmount(inv), coin: inv.coin })}
                      </button>
                    ) : user ? (
                      <>
                        <button
                          type="button"
                          disabled={
                            payBalanceMut.isPending ||
                            (walletsQ.isSuccess && walletUnits(inv.coin) < safeBigInt(inv.amount))
                          }
                          onClick={() => payBalanceMut.mutate()}
                          className="w-full rounded-2xl bg-gradient-to-r from-bitcoin to-bitcoin-dark py-3 px-4 text-xs font-bold text-white shadow-md shadow-bitcoin/25 hover:opacity-95 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          <i className="bi bi-lightning-charge-fill mr-1.5" />
                          {payBalanceMut.isPending
                            ? t('checkout.balance.processing')
                            : t('checkout.balance.payCoin', { amount: displayAmount(inv), coin: inv.coin })}
                        </button>
                        {walletsQ.isSuccess && walletUnits(inv.coin) < safeBigInt(inv.amount) && (
                          <p className="text-[11px] font-medium text-rose-600 text-center">{t('checkout.balance.short')}</p>
                        )}
                      </>
                    ) : (
                      <Link
                        to={loginTo}
                        className="block w-full rounded-2xl bg-gradient-to-r from-bitcoin to-bitcoin-dark py-3 px-4 text-xs font-bold text-white text-center shadow-md shadow-bitcoin/25 hover:opacity-95"
                      >
                        <i className="bi bi-box-arrow-in-right mr-1.5" />
                        {t('checkout.balance.signIn')}
                      </Link>
                    )}
                    {inv.demo && <p className="text-[11px] text-ink-muted text-center">{t('checkout.balance.showcase')}</p>}
                    {payBalanceMut.isError && (
                      <p className="text-[11px] font-medium text-rose-600 text-center">
                        {payBalanceMut.error instanceof ApiError && payBalanceMut.error.code === 'CANNOT_PAY_OWN_INVOICE'
                          ? t('checkout.balance.ownInvoice')
                          : t('checkout.balance.error')}
                      </p>
                    )}
                  </section>

                  <div className="flex items-center gap-3 text-[10px] font-bold uppercase tracking-wider text-ink-muted">
                    <span className="flex-1 border-t border-border" />
                    {t('checkout.crypto.or')}
                    <span className="flex-1 border-t border-border" />
                  </div>

                  <div className="flex flex-col items-center justify-center py-1">
                    <div className="flex h-48 w-48 items-center justify-center rounded-2xl border border-border bg-white p-3 shadow-md">
                      {qrUrl ? (
                        <img src={qrUrl} alt={t('checkout.crypto.qrAlt')} className="h-full w-full object-contain" />
                      ) : (
                        <div className="h-6 w-6 animate-spin rounded-full border-2 border-bitcoin border-t-transparent" />
                      )}
                    </div>
                    <span className="text-[10px] font-medium text-ink-muted mt-2">{t('checkout.crypto.scan')}</span>
                  </div>

                  <div>
                    <label className="block text-[10px] uppercase font-bold tracking-wider text-ink-muted mb-1.5 flex items-center justify-between">
                      <span>{t('checkout.crypto.address')}</span>
                      <span className="text-emerald-600 font-semibold lowercase">{t('checkout.crypto.officialNet')}</span>
                    </label>
                    <div className="flex items-center gap-2 rounded-2xl border border-border bg-surface p-2.5">
                      <div className="truncate font-mono text-xs text-ink flex-1 px-1 select-all">{inv.depositAddress}</div>
                      <button
                        type="button"
                        onClick={copyAddress}
                        className="shrink-0 rounded-xl bg-paper px-3 py-1.5 text-xs font-bold text-bitcoin-dark border border-border hover:bg-surface transition-all active:scale-95"
                      >
                        {copied ? (
                          <span className="text-emerald-600 flex items-center gap-1">
                            <i className="bi bi-check" /> {t('checkout.crypto.copied')}
                          </span>
                        ) : (
                          <span className="flex items-center gap-1">
                            <i className="bi bi-copy" /> {t('checkout.crypto.copy')}
                          </span>
                        )}
                      </button>
                    </div>
                  </div>

                  <div className="flex items-center justify-center gap-2 text-xs text-ink-muted pt-1">
                    <div className="h-3 w-3 animate-spin rounded-full border-2 border-bitcoin border-t-transparent" />
                    <span>{t('checkout.crypto.waiting')}</span>
                  </div>
                </>
              )}
            </div>
          )}
        </div>
      </main>

      <footer className="border-t border-border/70 py-4 text-center text-xs text-ink-muted bg-paper/50">
        <div className="mx-auto max-w-2xl px-4 flex items-center justify-between gap-3">
          <div className="flex items-center gap-1">
            <i className="bi bi-shield-lock-fill text-emerald-600" />
            <span>
              {t('checkout.footer.secure')} <b>SatsPay</b>
            </span>
          </div>
          <Link to="/" className="text-ink-muted hover:text-ink">
            {checkoutHost()}
          </Link>
        </div>
      </footer>
    </div>
  );
}

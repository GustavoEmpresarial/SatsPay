import { useMemo } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import { api } from '../lib/api.js';
import { coinLogo } from '../lib/coinAssets.js';
import { useAuthStore } from '../stores/auth.js';
import { COIN_CONFIG, COINS, asWalletBalances, formatAmount, formatPortfolioUsd, formatUsdValue, getCoinUsdValue, safeBigInt, type Coin, type WalletBalance } from '@/shared';

interface LedgerEntry {
  id: string;
  coin: Coin;
  amount: string;
  type: string;
  memo: string | null;
  createdAt: string;
}

function greetingKey(): 'greetingMorning' | 'greetingAfternoon' | 'greetingEvening' {
  const h = new Date().getHours();
  if (h < 12) return 'greetingMorning';
  if (h < 18) return 'greetingAfternoon';
  return 'greetingEvening';
}

const CREDIT_TYPES = new Set(['DEPOSIT', 'FAUCET', 'TRANSFER_IN', 'WITHDRAWAL_REVERSAL', 'ADJUSTMENT']);

export function DashboardPage() {
  const { t, i18n } = useTranslation();
  const user = useAuthStore((s) => s.user);
  const displayName = user?.username?.trim() || user?.email?.split('@')[0] || '';

  const walletsQ = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<WalletBalance[] | { wallets: WalletBalance[] }>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
  });

  const pricesQ = useQuery({
    queryKey: ['prices'],
    queryFn: () => api<{ prices: Record<string, string>; priceDecimals: number }>('/swap/prices', { skipAuth: true }),
    staleTime: 30_000,
    refetchInterval: 60_000,
  });

  const ledgerQ = useQuery({
    queryKey: ['ledger', 'recent'],
    queryFn: () => api<{ entries: LedgerEntry[] }>('/wallet/ledger?take=15'),
    enabled: Boolean(user),
  });

  const defaultWallets: WalletBalance[] = COINS.map((c) => ({
    coin: c,
    balance: '0',
    kind: 'PERSONAL',
  }));
  const rawList = asWalletBalances(walletsQ.data);
  const wallets = rawList.length ? rawList : defaultWallets;
  const coinsWithBalance = useMemo(() => wallets.filter((w) => safeBigInt(w.balance) > 0n).length, [wallets]);
  const twoFaOn = Boolean(user?.twoFactorEnabled);

  const totalBalanceUsd = useMemo(() => {
    return wallets.reduce((sum, w) => {
      return sum + getCoinUsdValue(w.balance, w.coin as Coin, pricesQ.data?.prices, pricesQ.data?.priceDecimals);
    }, 0);
  }, [wallets, pricesQ.data]);

  const totalBalanceUsdFormatted = formatPortfolioUsd(totalBalanceUsd, coinsWithBalance > 0);

  const dateFmt = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? 'en', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

  return (
    <div className="space-y-5 md:space-y-6">
      {/* GREETING BAR */}
      <motion.div
        initial={{ opacity: 0, y: -8 }}
        animate={{ opacity: 1, y: 0 }}
        className="flex flex-wrap items-end justify-between gap-3"
      >
        <div>
          <h1 className="text-xl font-bold tracking-tight sm:text-2xl md:text-3xl">
            {t(`dashboard.${greetingKey()}`)},{' '}
            <span className="text-bitcoin-dark">{displayName}</span>
          </h1>
          <p className="text-sm text-ink-muted">{t('dashboard.yourWallet')}</p>
        </div>
        <div className="hidden items-center gap-2 sm:flex">
          <div className="rounded-full border border-border bg-paper px-3 py-1.5 text-xs text-ink-muted">
            <i className={`bi ${twoFaOn ? 'bi-shield-check text-emerald-600' : 'bi-shield text-ink-muted'} mr-1`} />
            {twoFaOn ? t('dashboard.twoFaOn') : t('dashboard.twoFaOff')}
          </div>
        </div>
      </motion.div>

      {/* HERO + QUICK ACTIONS */}
      <div className="grid gap-4 lg:grid-cols-12">
        {/* Big hero balance — spans 8/12 on lg, full on smaller */}
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.05 }}
          className="relative overflow-hidden rounded-2xl bg-gradient-to-br from-bitcoin via-bitcoin to-bitcoin-dark p-5 text-white shadow-xl shadow-bitcoin/20 md:p-7 lg:col-span-8"
        >
          <div
            className="pointer-events-none absolute inset-0 opacity-15"
            style={{
              backgroundImage: 'radial-gradient(circle at 1px 1px, white 1px, transparent 0)',
              backgroundSize: '24px 24px',
            }}
          />
          <div className="pointer-events-none absolute -right-16 -top-16 h-56 w-56 rounded-full bg-white/10 blur-3xl" />
          <div className="pointer-events-none absolute -bottom-20 -left-16 h-64 w-64 rounded-full bg-black/10 blur-3xl" />

          <div className="relative flex h-full flex-col">
            <div className="mb-4 flex items-center justify-between">
              <div>
                <div className="mb-1 flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-widest text-white/70">
                  <i className="bi bi-wallet2" />
                  {t('dashboard.yourWallet')}
                </div>
                <div className="text-3xl font-bold sm:text-4xl md:text-5xl">{totalBalanceUsdFormatted}</div>
                <p className="mt-1 text-sm text-white/80">
                  {coinsWithBalance > 0
                    ? t('dashboard.fundedCoins', { n: coinsWithBalance })
                    : t('dashboard.emptyWallet')}
                </p>
              </div>
              <div className="hidden text-right md:block">
                <div className="mt-1 flex items-center justify-end gap-1 text-xs text-white/90">
                  <i className={`bi ${twoFaOn ? 'bi-shield-check' : 'bi-shield'}`} />{' '}
                  {twoFaOn ? t('dashboard.twoFaOn') : t('dashboard.twoFaOff')}
                </div>
              </div>
            </div>

            <div className="mt-auto grid grid-cols-2 gap-2 sm:grid-cols-4">
              {wallets.map((w) => {
                const cfg = COIN_CONFIG[w.coin as Coin];
                return (
                  <Link
                    key={w.coin}
                    to={`/deposit?coin=${w.coin}`}
                    className="rounded-xl border border-white/10 bg-white/10 p-3 backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:bg-white/15"
                  >
                    <div className="mb-1 flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-white/80">
                      <img src={coinLogo(w.coin as Coin)} alt={w.coin} className="h-4 w-4 rounded-full" /> {cfg.symbol}
                    </div>
                    <div className="truncate font-mono text-sm font-bold sm:text-base">
                      {formatAmount(w.balance, w.coin as Coin)}
                    </div>
                    <div className="text-[10px] text-white/70 truncate">
                      ≈ {formatUsdValue(w.balance, w.coin as Coin, pricesQ.data?.prices, pricesQ.data?.priceDecimals)}
                    </div>
                  </Link>
                );
              })}
            </div>
          </div>
        </motion.div>

        {/* Quick actions — 4/12 on lg, full on smaller (row on mobile) */}
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
          className="card flex flex-col p-4 md:p-5 lg:col-span-4"
        >
          <div className="mb-3 text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t('dashboard.quickActions')}
          </div>
          <div className="grid flex-1 grid-cols-3 gap-2 lg:grid-cols-1">
            {[
              { to: '/deposit', icon: 'bi-arrow-down-circle-fill', bg: 'bg-emerald-100', color: 'text-emerald-700', label: t('dashboard.deposit'), hint: t('dashboard.hints.deposit') },
              { to: '/withdraw', icon: 'bi-arrow-up-circle-fill', bg: 'bg-blue-100', color: 'text-blue-700', label: t('dashboard.withdraw'), hint: t('dashboard.hints.withdraw') },
              { to: '/faucet', icon: 'bi-droplet-fill', bg: 'bg-bitcoin/15', color: 'text-bitcoin-dark', label: t('dashboard.claimFaucet'), hint: t('dashboard.hints.faucet') },
            ].map((a) => (
              <Link
                key={a.to}
                to={a.to}
                className="group flex flex-col items-center gap-1 rounded-lg border border-border p-3 text-center transition-all hover:border-bitcoin/50 hover:bg-bitcoin/5 lg:flex-row lg:text-left"
              >
                <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${a.bg} ${a.color}`}>
                  <i className={`bi ${a.icon} text-lg`} />
                </div>
                <div className="hidden flex-1 lg:block">
                  <div className="text-sm font-semibold">{a.label}</div>
                  <div className="text-xs text-ink-muted">{a.hint}</div>
                </div>
                <div className="text-xs font-medium lg:hidden">{a.label}</div>
                <i className="bi bi-chevron-right ml-auto hidden text-ink-muted transition-transform group-hover:translate-x-0.5 lg:inline" />
              </Link>
            ))}
          </div>
        </motion.div>
      </div>

      {/* WALLETS + ACTIVITY — side by side on xl */}
      <div className="grid gap-5 xl:grid-cols-12">
        {/* Wallets grid — 7/12 on xl */}
        <section className="xl:col-span-7">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">
              {t('dashboard.balances')}
            </h2>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            {walletsQ.isLoading &&
              Array.from({ length: 4 }).map((_, i) => (
                <div key={i} className="card h-44 animate-pulse-slow bg-surface" />
              ))}
            {wallets.map((w, i) => {
              const cfg = COIN_CONFIG[w.coin as Coin];
              const balance = safeBigInt(w.balance);
              const hasBalance = balance > 0n;
              return (
                <motion.div
                  key={w.coin}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.04 * i }}
                  className="card group relative overflow-hidden p-4 transition-shadow hover:shadow-lg md:p-5"
                >
                  <div
                    className="pointer-events-none absolute -right-12 -top-12 h-32 w-32 rounded-full opacity-10 blur-2xl transition-transform group-hover:scale-125"
                    style={{ background: cfg.displayColor }}
                  />
                  <div className="relative">
                    <div className="mb-3 flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <div
                          className="flex h-10 w-10 items-center justify-center rounded-xl bg-paper p-1 shadow-md ring-1 ring-border"
                          style={{ boxShadow: `0 6px 16px ${cfg.displayColor}44` }}
                        >
                          <img src={coinLogo(w.coin as Coin)} alt={w.coin} className="h-full w-full rounded-full" />
                        </div>
                        <div>
                          <div className="text-sm font-semibold leading-tight">{cfg.name}</div>
                          <div className="text-[10px] uppercase tracking-wider text-ink-muted">{cfg.symbol}</div>
                        </div>
                      </div>
                      {hasBalance && (
                        <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-[10px] font-semibold text-emerald-700">
                          <i className="bi bi-check-circle-fill" />
                        </span>
                      )}
                    </div>

                    <div className="font-mono text-2xl font-bold leading-tight md:text-[26px]">
                      {formatAmount(balance, w.coin as Coin)}
                    </div>
                    <div className="mt-0.5 text-xs font-semibold text-ink-muted">
                      ≈ {formatUsdValue(balance, w.coin as Coin, pricesQ.data?.prices, pricesQ.data?.priceDecimals)} USD
                    </div>

                    <div className="mt-3 flex gap-2">
                      <Link
                        to={`/deposit?coin=${w.coin}`}
                        className="flex flex-1 items-center justify-center gap-1 rounded-lg border border-border py-1.5 text-xs font-medium hover:border-bitcoin/50 hover:bg-bitcoin/5"
                      >
                        <i className="bi bi-arrow-down" /> {t('dashboard.actionDeposit')}
                      </Link>
                      <Link
                        to={`/withdraw?coin=${w.coin}`}
                        className="flex flex-1 items-center justify-center gap-1 rounded-lg border border-border py-1.5 text-xs font-medium hover:border-bitcoin/50 hover:bg-bitcoin/5"
                      >
                        <i className="bi bi-arrow-up" /> {t('dashboard.actionSend')}
                      </Link>
                    </div>
                  </div>
                </motion.div>
              );
            })}
          </div>
        </section>

        {/* Activity — 5/12 on xl, full width below xl */}
        <section className="xl:col-span-5">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">
              {t('dashboard.recentActivity')}
            </h2>
          </div>

          <div className="card overflow-hidden">
            {ledgerQ.isLoading && (
              <div className="space-y-2 p-4">
                {Array.from({ length: 4 }).map((_, i) => (
                  <div key={i} className="h-12 animate-pulse-slow rounded-lg bg-surface" />
                ))}
              </div>
            )}

            {!ledgerQ.isLoading && (!ledgerQ.data || ledgerQ.data.entries.length === 0) && (
              <div className="p-8 text-center">
                <div className="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-full bg-bitcoin/10 text-2xl text-bitcoin-dark">
                  <i className="bi bi-inbox" />
                </div>
                <h3 className="font-semibold text-ink text-sm mb-1">{t('dashboard.noActivity', { defaultValue: 'Nenhuma atividade recente' })}</h3>
                <p className="mb-4 text-xs text-ink-muted leading-relaxed max-w-xs mx-auto">
                  Suas transações, depósitos, saques e reivindicações de faucet aparecerão aqui.
                </p>
                <Link to="/faucet" className="btn-primary inline-flex text-xs py-2 px-4">
                  <i className="bi bi-droplet-fill mr-1.5" /> {t('dashboard.claimFaucet', { defaultValue: 'Reivindicar Faucet' })}
                </Link>
              </div>
            )}

            {ledgerQ.data && ledgerQ.data.entries.length > 0 && (
              <ul className="max-h-[520px] divide-y divide-border overflow-y-auto">
                {(() => {
                  // Merge WITHDRAWAL + WITHDRAWAL_FEE into a single row
                  const merged: Array<{ entry: LedgerEntry; feeEntry?: LedgerEntry }> = [];
                  const entries = ledgerQ.data!.entries;
                  const skip = new Set<string>();

                  for (let i = 0; i < entries.length; i++) {
                    const e = entries[i]!;
                    if (skip.has(e.id)) continue;

                    if (e.type === 'WITHDRAWAL' && i + 1 < entries.length) {
                      const next = entries[i + 1]!;
                      if (next.type === 'WITHDRAWAL_FEE' && next.coin === e.coin) {
                        merged.push({ entry: e, feeEntry: next });
                        skip.add(next.id);
                        continue;
                      }
                    }
                    merged.push({ entry: e });
                  }

                  return merged.map(({ entry: e, feeEntry }) => {
                    const amount = safeBigInt(e.amount);
                    const isCredit = CREDIT_TYPES.has(e.type) || amount > 0n;

                    // Compute total for merged withdrawal
                    const feeAmount = feeEntry ? safeBigInt(feeEntry.amount) : 0n;
                    const totalAmount = amount < 0n ? -((-amount) + (feeAmount < 0n ? -feeAmount : 0n)) : amount;
                    const displayAmount = totalAmount < 0n ? -totalAmount : totalAmount;

                    return (
                      <li key={e.id} className="flex items-center gap-3 px-4 py-3 hover:bg-surface md:px-5">
                        <div className="relative shrink-0">
                          <img src={coinLogo(e.coin)} alt={e.coin} className="h-9 w-9 rounded-full" />
                          <div
                            className={`absolute -bottom-0.5 -right-0.5 flex h-4 w-4 items-center justify-center rounded-full ring-2 ring-paper ${
                              isCredit ? 'bg-emerald-500 text-white' : 'bg-rose-500 text-white'
                            }`}
                          >
                            <i className={`bi ${isCredit ? 'bi-arrow-down-left' : 'bi-arrow-up-right'} text-[8px]`} />
                          </div>
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <span className="truncate text-sm font-medium">
                              {t(`dashboard.type.${e.type}`, { defaultValue: e.type })}
                            </span>
                            <span className="text-[10px] font-semibold text-ink-muted">{e.coin}</span>
                          </div>
                          <div className="flex items-center gap-2 text-[10px] text-ink-muted">
                            <span>{dateFmt.format(new Date(e.createdAt))}</span>
                            {feeEntry && (
                              <span className="text-ink-muted">
                                (taxa: {formatAmount(feeAmount < 0n ? -feeAmount : feeAmount, e.coin)})
                              </span>
                            )}
                          </div>
                        </div>
                        <div
                          className={`shrink-0 text-right font-mono text-sm font-semibold ${
                            isCredit ? 'text-emerald-700' : 'text-rose-700'
                          }`}
                        >
                          {totalAmount > 0n ? '+' : '-'}
                          {formatAmount(displayAmount, e.coin)}
                        </div>
                      </li>
                    );
                  });
                })()}
              </ul>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COIN_CONFIG, COINS, formatAmountFixed, formatUsdValue, safeBigInt, type WalletBalance } from '@/shared';
import { Link } from 'react-router-dom';

interface WalletsResp {
  wallets: WalletBalance[];
}

export function WalletsPage() {
  const { t } = useTranslation();
  const user = useAuthStore((s) => s.user);
  const { data, isLoading } = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<WalletsResp>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
  });

  const pricesQ = useQuery({
    queryKey: ['prices'],
    queryFn: () => api<{ prices: Record<string, string>; priceDecimals: number }>('/swap/prices', { skipAuth: true }),
    staleTime: 30_000,
    refetchInterval: 60_000,
  });

  const list = Array.isArray(data) ? data : data?.wallets ?? [];
  const walletMap = list.reduce<Record<string, WalletBalance>>((acc, w) => {
    acc[w.coin] = w;
    return acc;
  }, {});

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-bold tracking-tight">{t('wallets.title')}</h1>
        <p className="text-sm text-ink-muted mt-1">{t('wallets.subtitle')}</p>
      </header>

      {isLoading ? (
        <div className="grid gap-3 sm:gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3">
          {COINS.map((c) => (
            <div key={c} className="card h-48 animate-pulse p-4 sm:p-5" />
          ))}
        </div>
      ) : (
        <div className="grid gap-3 sm:gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3">
          {COINS.map((coin) => {
            const cfg = COIN_CONFIG[coin];
            const w = walletMap[coin];
            const bal = w ? safeBigInt(w.balance) : 0n;

            return (
              <div key={coin} className="card p-4 sm:p-5 flex flex-col justify-between shadow-xs hover:shadow-md transition-shadow">
                <div>
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <img src={coinLogo(coin)} alt={coin} className="h-9 w-9 rounded-full object-contain shrink-0" />
                      <div>
                        <h3 className="font-bold text-sm text-ink">{cfg.name}</h3>
                        <span className="text-[11px] font-mono text-ink-muted uppercase">{coin}</span>
                      </div>
                    </div>
                    <span className="rounded-full bg-surface border border-border px-2.5 py-0.5 text-[10px] font-bold text-ink-muted">
                      {cfg.minConfirmations} confs
                    </span>
                  </div>

                  <div className="mt-4 rounded-xl bg-surface p-3.5 border border-border space-y-1">
                    <div className="text-[10px] uppercase font-bold tracking-wider text-ink-muted">
                      {t('wallets.balance')}
                    </div>
                    <div className="font-mono text-xl sm:text-2xl font-black text-ink truncate">
                      {formatAmountFixed(bal, coin)} <span className="text-xs text-ink-muted font-bold">{coin}</span>
                    </div>
                    <div className="text-xs font-semibold text-ink-muted">
                      ≈ {formatUsdValue(bal, coin, pricesQ.data?.prices, pricesQ.data?.priceDecimals)} USD
                    </div>
                  </div>
                </div>

                <div className="mt-4 flex gap-2 pt-1">
                  <Link to={`/deposit?coin=${coin}`} className="btn-secondary flex-1 text-center text-xs font-bold py-2.5 rounded-xl">
                    <i className="bi bi-arrow-down-left mr-1" />
                    {t('nav.deposit')}
                  </Link>
                  <Link to={`/withdraw?coin=${coin}`} className="btn-secondary flex-1 text-center text-xs font-bold py-2.5 rounded-xl">
                    <i className="bi bi-arrow-up-right mr-1" />
                    {t('nav.withdraw')}
                  </Link>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

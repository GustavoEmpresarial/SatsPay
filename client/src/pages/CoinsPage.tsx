import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { COIN_CONFIG, COINS, type Coin } from '@/shared';
import { MarketingPage } from '../components/MarketingPage.js';
import { coinLogo } from '../lib/coinAssets.js';

export function CoinsPage() {
  const { t } = useTranslation();

  return (
    <MarketingPage
      eyebrow={t('landing.coins.kicker')}
      title={t('landing.coins.title')}
      subtitle={t('pages.coins.subtitle')}
    >
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-3 lg:grid-cols-4">
        {COINS.map((c: Coin) => {
          const cfg = COIN_CONFIG[c];
          return (
            <div
              key={c}
              className="flex flex-col items-center gap-3 rounded-2xl border border-border bg-paper p-6 text-center shadow-xs hover:border-bitcoin/40 hover:shadow-md transition-all group"
            >
              <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-surface/80 p-2 shadow-sm border border-border/70 group-hover:scale-105 transition-transform">
                <img
                  src={coinLogo(c)}
                  alt={cfg.name}
                  className="h-full w-full rounded-full object-contain"
                />
              </div>
              <div>
                <div className="text-base font-bold text-ink">{cfg.name}</div>
                <div className="text-xs uppercase tracking-wider font-semibold text-bitcoin-dark">{cfg.symbol}</div>
              </div>
              <div className="flex items-center gap-1 text-[11px] text-ink-muted">
                <i className="bi bi-check2-circle text-emerald-600 font-bold" />
                <span>{t('landing.coins.trio')}</span>
              </div>
            </div>
          );
        })}
      </div>
      <div className="mt-8 flex items-center justify-between">
        <Link to="/features" className="text-sm font-medium text-bitcoin-dark hover:underline">
          {t('footer.links.features')} →
        </Link>
        <Link to="/faucet" className="btn-primary text-xs px-4 py-2">
          {t('landing.nav.faucet', { defaultValue: 'Testar Torneira (Faucet)' })} →
        </Link>
      </div>
    </MarketingPage>
  );
}

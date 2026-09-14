import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { MarketingPage } from '../components/MarketingPage.js';

const featureItems = [
  { icon: 'bi-droplet-fill', key: 'faucet' },
  { icon: 'bi-code-slash', key: 'api' },
  { icon: 'bi-shield-lock-fill', key: 'twofa' },
  { icon: 'bi-journal-check', key: 'ledger' },
  { icon: 'bi-person-check-fill', key: 'approval' },
  { icon: 'bi-globe2', key: 'multi' },
] as const;

export function FeaturesPage() {
  const { t } = useTranslation();

  return (
    <MarketingPage
      eyebrow={t('landing.features.kicker')}
      title={t('landing.features.title')}
      subtitle={t('landing.features.subtitle')}
    >
      <div className="grid gap-5 sm:grid-cols-2">
        {featureItems.map((f) => (
          <div
            key={f.key}
            className="rounded-xl border border-border bg-paper p-6 transition hover:border-bitcoin/40"
          >
            <div className="mb-4 inline-flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-bitcoin to-bitcoin-dark text-xl text-white shadow-lg shadow-bitcoin/30">
              <i className={`bi ${f.icon}`} />
            </div>
            <h2 className="mb-2 text-lg font-semibold text-ink">{t(`landing.features.${f.key}.title`)}</h2>
            <p className="text-sm leading-relaxed text-ink-muted">{t(`landing.features.${f.key}.desc`)}</p>
          </div>
        ))}
      </div>
      <div className="mt-8 flex flex-wrap gap-3">
        <Link
          to="/register"
          className="rounded-lg bg-bitcoin px-4 py-2.5 text-sm font-semibold text-white hover:bg-bitcoin-dark"
        >
          {t('common.getStartedFree')}
        </Link>
        <Link
          to="/coins"
          className="rounded-lg border border-border px-4 py-2.5 text-sm font-medium text-ink hover:border-bitcoin/40"
        >
          {t('footer.links.coins')}
        </Link>
      </div>
    </MarketingPage>
  );
}

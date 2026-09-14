import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { MarketingPage } from '../components/MarketingPage.js';

export function GuidesPage() {
  const { t } = useTranslation();
  const guides = t('pages.guides.items', { returnObjects: true }) as Array<{
    title: string;
    desc: string;
    to: string;
  }>;

  return (
    <MarketingPage
      eyebrow={t('footer.developers')}
      title={t('pages.guides.title')}
      subtitle={t('pages.guides.subtitle')}
    >
      <div className="grid gap-4 sm:grid-cols-2">
        {Array.isArray(guides) &&
          guides.map((g) => (
            <Link
              key={g.to}
              to={g.to}
              className="rounded-xl border border-border bg-paper p-5 transition hover:border-bitcoin/40 hover:shadow-md"
            >
              <h2 className="font-semibold text-ink">{g.title}</h2>
              <p className="mt-1.5 text-sm text-ink-muted">{g.desc}</p>
              <span className="mt-3 inline-block text-sm font-medium text-bitcoin-dark">→</span>
            </Link>
          ))}
      </div>
    </MarketingPage>
  );
}

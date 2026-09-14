import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { MarketingPage } from '../components/MarketingPage.js';

export function SecurityPage() {
  const { t } = useTranslation();
  const sections = t('pages.security.sections', { returnObjects: true }) as Array<{
    title: string;
    body: string;
  }>;

  return (
    <MarketingPage
      eyebrow={t('footer.links.security')}
      title={t('pages.security.title')}
      subtitle={t('pages.security.subtitle')}
    >
      <div className="space-y-6">
        {Array.isArray(sections) &&
          sections.map((s) => (
            <section key={s.title} className="rounded-xl border border-border bg-paper p-5">
              <h2 className="mb-2 font-semibold text-ink">{s.title}</h2>
              <p className="text-sm leading-relaxed text-ink-muted">{s.body}</p>
            </section>
          ))}
      </div>
      <div className="mt-8 flex flex-wrap gap-4 text-sm">
        <a href="mailto:security@bitcosats.com" className="font-medium text-bitcoin-dark hover:underline">
          security@bitcosats.com
        </a>
        <a href="/security.txt" className="text-ink-muted hover:text-ink">
          security.txt
        </a>
        <Link to="/privacy" className="text-ink-muted hover:text-ink">
          {t('footer.links.privacy')}
        </Link>
      </div>
    </MarketingPage>
  );
}

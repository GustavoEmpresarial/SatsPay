import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { MarketingPage } from '../components/MarketingPage.js';

export function LlmPage() {
  const { t } = useTranslation();

  return (
    <MarketingPage eyebrow={t('pages.llm.eyebrow')} title={t('pages.llm.title')} subtitle={t('pages.llm.subtitle')}>
      <div className="space-y-6 text-sm leading-relaxed text-ink-muted">
        <p>{t('pages.llm.intro')}</p>
        <section className="rounded-xl border border-border bg-paper p-5">
          <h2 className="mb-2 font-semibold text-ink">{t('pages.llm.productTitle')}</h2>
          <ul className="list-disc space-y-1 pl-5">
            <li>
              <Link to="/welcome" className="text-bitcoin-dark hover:underline">
                /welcome
              </Link>
            </li>
            <li>
              <Link to="/documentation" className="text-bitcoin-dark hover:underline">
                /documentation
              </Link>
            </li>
            <li>
              <Link to="/api" className="text-bitcoin-dark hover:underline">
                /api
              </Link>
            </li>
            <li>
              <span className="font-mono text-xs">https://api.bitcosats.com/v1</span>
            </li>
          </ul>
        </section>
        <section className="rounded-xl border border-border bg-paper p-5">
          <h2 className="mb-2 font-semibold text-ink">{t('pages.llm.devTitle')}</h2>
          <ul className="list-disc space-y-1 pl-5">
            <li>{t('pages.llm.dev1')}</li>
            <li>{t('pages.llm.dev2')}</li>
            <li>{t('pages.llm.dev3')}</li>
          </ul>
        </section>
        <p className="text-xs">
          {t('pages.llm.rawHint')}{' '}
          <a href="/llm.txt" className="font-mono text-bitcoin-dark hover:underline">
            /llm.txt
          </a>
        </p>
      </div>
    </MarketingPage>
  );
}

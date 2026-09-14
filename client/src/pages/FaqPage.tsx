import { useTranslation } from 'react-i18next';
import { MarketingPage } from '../components/MarketingPage.js';

export function FaqPage() {
  const { t } = useTranslation();
  const faqItems = t('landing.faq.items', { returnObjects: true }) as Array<{ q: string; a: string }>;

  return (
    <MarketingPage
      eyebrow={t('landing.faq.kicker')}
      title={t('landing.faq.title')}
      subtitle={t('pages.faq.subtitle')}
    >
      <div className="mx-auto max-w-3xl space-y-3">
        {Array.isArray(faqItems) &&
          faqItems.map((item) => (
            <details key={item.q} className="group overflow-hidden rounded-xl border border-border bg-paper">
              <summary className="flex cursor-pointer list-none items-center justify-between px-5 py-4 font-semibold text-ink">
                <span className="flex items-center gap-3 pr-4">
                  <i className="bi bi-patch-question-fill text-bitcoin-dark" />
                  {item.q}
                </span>
                <i className="bi bi-chevron-down shrink-0 transition-transform group-open:rotate-180" />
              </summary>
              <div className="px-5 pb-5 text-sm leading-relaxed text-ink-muted">{item.a}</div>
            </details>
          ))}
      </div>
    </MarketingPage>
  );
}

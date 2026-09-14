import { useTranslation } from 'react-i18next';
import { MarketingPage } from './MarketingPage.js';

interface LegalDocumentProps {
  titleKey: string;
  updatedKey?: string;
  sectionsKey: string;
}

export function LegalDocument({ titleKey, updatedKey, sectionsKey }: LegalDocumentProps) {
  const { t } = useTranslation();
  const sections = t(sectionsKey, { returnObjects: true }) as Array<{ title?: string; body?: string }> | Record<string, { title?: string; body?: string }>;

  const sectionsList = Array.isArray(sections) ? sections : Object.values(sections || {});

  return (
    <MarketingPage title={t(titleKey)} subtitle={updatedKey ? t(updatedKey) : undefined}>
      <div className="space-y-8 text-ink-muted leading-relaxed">
        {sectionsList.map((s, idx) => (
          <div key={idx} className="space-y-2">
            {s.title && <h2 className="text-lg font-semibold text-ink">{s.title}</h2>}
            {s.body && <p className="whitespace-pre-line">{s.body}</p>}
          </div>
        ))}
      </div>
    </MarketingPage>
  );
}

import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

type Props = {
  eyebrow?: string;
  title: string;
  subtitle?: string;
  children: React.ReactNode;
};

export function MarketingPage({ eyebrow, title, subtitle, children }: Props) {
  const { t } = useTranslation();

  return (
    <div className="mx-auto max-w-5xl px-4 py-12 md:px-6 md:py-16">
      <p className="mb-3 text-xs font-semibold uppercase tracking-[0.16em] text-bitcoin-dark">
        {eyebrow ?? 'SatsPay'}
      </p>
      <h1 className="text-3xl font-semibold tracking-tight text-ink md:text-4xl">{title}</h1>
      {subtitle && <p className="mt-3 max-w-2xl text-base leading-relaxed text-ink-muted">{subtitle}</p>}
      <div className="mt-10">{children}</div>
      <div className="mt-12 border-t border-border pt-6">
        <Link to="/" className="text-sm text-ink-muted hover:text-bitcoin-dark">
          ← {t('footer.backHome')}
        </Link>
      </div>
    </div>
  );
}

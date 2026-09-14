import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LanguageSwitch } from './LanguageSwitch.js';
import { ThemeToggle } from './ThemeToggle.js';

export function SiteFooter() {
  const { t } = useTranslation();
  const year = new Date().getFullYear();

  return (
    <footer className="border-t border-border bg-paper text-ink">
      <div className="mx-auto max-w-6xl px-4 py-12 md:px-6">
        <div className="grid grid-cols-2 gap-8 md:grid-cols-5">
          <div className="col-span-2">
            <div className="flex items-center gap-3">
              <img
                src="/logo.png"
                alt="SatsPay"
                className="h-9 w-9 object-contain drop-shadow-sm shrink-0"
              />
              <span className="font-extrabold tracking-tight text-lg text-ink">SatsPay</span>
            </div>
            <p className="mt-3 max-w-sm text-sm text-ink-muted leading-relaxed">
              {t('footer.tagline')}
            </p>
            <div className="mt-4 flex flex-wrap items-center gap-2">
              <ThemeToggle />
              <LanguageSwitch />
            </div>
          </div>

          <div>
            <h4 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
              {t('footer.product')}
            </h4>
            <ul className="mt-3 space-y-2 text-sm text-ink-muted">
              <li><Link to="/features" className="hover:text-ink">{t('footer.links.features')}</Link></li>
              <li><Link to="/coins" className="hover:text-ink">{t('footer.links.coins')}</Link></li>
            </ul>
          </div>

          <div>
            <h4 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
              {t('footer.developers')}
            </h4>
            <ul className="mt-3 space-y-2 text-sm text-ink-muted">
              <li><Link to="/documentation" className="hover:text-ink">{t('footer.links.docs')}</Link></li>
              <li><Link to="/status" className="hover:text-ink">Status</Link></li>
              <li><Link to="/api" className="hover:text-ink">{t('footer.links.api')}</Link></li>
              <li><Link to="/guides" className="hover:text-ink">{t('footer.links.guides')}</Link></li>
              <li><Link to="/llm" className="hover:text-ink">{t('footer.links.llm')}</Link></li>
            </ul>
          </div>

          <div>
            <h4 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
              {t('footer.legal')}
            </h4>
            <ul className="mt-3 space-y-2 text-sm text-ink-muted">
              <li><Link to="/privacy" className="hover:text-ink">{t('footer.links.privacy')}</Link></li>
              <li><Link to="/terms" className="hover:text-ink">{t('footer.links.terms')}</Link></li>
              <li><Link to="/cookies" className="hover:text-ink">{t('footer.links.cookies')}</Link></li>
              <li><Link to="/security" className="hover:text-ink">{t('footer.links.security')}</Link></li>
              <li><Link to="/faq" className="hover:text-ink">{t('footer.links.faq')}</Link></li>
              <li><Link to="/support" className="hover:text-ink">{t('footer.links.support')}</Link></li>
              <li><Link to="/sitemap" className="hover:text-ink">{t('footer.links.sitemap')}</Link></li>
            </ul>
          </div>
        </div>

        <div className="mt-12 flex flex-col items-center justify-between gap-4 border-t border-border pt-6 text-xs text-ink-muted sm:flex-row">
          <p>© {year} SatsPay. {t('footer.rights')}</p>
          <p>{t('footer.compliance')}</p>
        </div>
      </div>
    </footer>
  );
}

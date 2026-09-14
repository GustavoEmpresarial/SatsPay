import { Link, Outlet } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { SiteFooter } from './SiteFooter.js';
import { LanguageSwitch } from './LanguageSwitch.js';
import { ThemeToggle } from './ThemeToggle.js';
import { CookieConsent } from './CookieConsent.js';

export function MarketingLayout() {
  const { t } = useTranslation();

  return (
    <div className="flex min-h-screen flex-col bg-canvas text-ink">
      <header className="sticky top-0 z-40 border-b border-border/80 bg-paper/90 backdrop-blur-md">
        <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-4 md:px-6">
          <Link to="/" className="flex items-center gap-3">
            <img
              src="/logo.png"
              alt="SatsPay"
              className="h-9 w-9 object-contain drop-shadow-sm shrink-0 transition-transform hover:scale-105"
            />
            <span className="font-extrabold tracking-tight text-lg text-ink">SatsPay</span>
          </Link>
          <div className="flex items-center gap-2 sm:gap-3">
            <ThemeToggle />
            <LanguageSwitch />
            <Link to="/login" className="rounded-xl px-3.5 py-1.5 text-sm font-semibold text-ink-muted hover:text-ink hover:bg-canvas transition-colors">
              {t('common.signIn')}
            </Link>
            <Link
              to="/register"
              className="inline-flex items-center gap-2 rounded-xl bg-bitcoin px-4 py-1.5 text-sm font-semibold text-white shadow-sm shadow-bitcoin/25 hover:bg-bitcoin-dark transition-all hover:scale-105 active:scale-95"
            >
              {t('common.signUp')}
            </Link>
          </div>
        </div>
      </header>
      <main className="flex-1">
        <Outlet />
      </main>
      <SiteFooter />
      <CookieConsent />
    </div>
  );
}

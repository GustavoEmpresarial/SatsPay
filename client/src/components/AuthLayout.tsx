import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import type { ReactNode } from 'react';
import { LanguageSwitch } from './LanguageSwitch.js';
import { ThemeToggle } from './ThemeToggle.js';
import { COINS, COIN_CONFIG } from '@/shared';
import { coinLogo } from '../lib/coinAssets.js';

export function AuthLayout({
  title,
  subtitle,
  children,
  footer,
  side = 'default',
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  footer?: ReactNode;
  side?: 'default' | 'register';
}) {
  const { t } = useTranslation();
  const register = side === 'register';

  return (
    <div className="min-h-screen bg-surface">
      <header className="flex items-center justify-between px-4 py-4 md:px-8">
        <Link
          to="/welcome"
          className="inline-flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm font-medium text-ink-muted transition-colors hover:bg-paper hover:text-ink"
        >
          <i className="bi bi-arrow-left" />
          <span className="hidden sm:inline">{t('auth.layout.backHome')}</span>
        </Link>
        <div className="flex items-center gap-2">
          <ThemeToggle />
          <LanguageSwitch />
        </div>
      </header>

      <div className="mx-auto grid min-h-[calc(100vh-72px)] max-w-6xl grid-cols-1 items-center gap-8 px-4 pb-12 lg:grid-cols-2 lg:gap-16 lg:px-8">
        <motion.aside
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          className="hidden lg:block"
        >
          <div className="relative overflow-hidden rounded-3xl bg-gradient-to-br from-bitcoin via-bitcoin to-bitcoin-dark p-8 text-white shadow-2xl shadow-bitcoin/30">
            <div
              className="pointer-events-none absolute inset-0 opacity-10"
              style={{
                backgroundImage: 'radial-gradient(circle at 1px 1px, white 1px, transparent 0)',
                backgroundSize: '24px 24px',
              }}
            />
            <div className="pointer-events-none absolute -right-16 -top-16 h-64 w-64 rounded-full bg-white/10 blur-3xl" />

            <div className="relative">
              <Link to="/welcome" className="mb-8 inline-flex items-center gap-3">
                <img
                  src="/logo.png"
                  alt="SatsPay"
                  className="h-11 w-11 object-contain drop-shadow-xl shrink-0"
                />
                <span className="text-2xl font-bold tracking-tight">SatsPay</span>
              </Link>

              <h2 className="mb-3 text-3xl font-bold leading-tight">
                {t(register ? 'auth.layout.registerHeadline' : 'auth.layout.loginHeadline')}
              </h2>
              <p className="mb-8 text-white/85">
                {t(register ? 'auth.layout.registerBody' : 'auth.layout.loginBody')}
              </p>

              <div className="mb-6 grid grid-cols-5 gap-2">
                {COINS.map((c) => (
                  <div
                    key={c}
                    className="flex flex-col items-center gap-1 rounded-xl bg-white/10 p-2 backdrop-blur-sm"
                  >
                    <img src={coinLogo(c)} alt={c} className="h-8 w-8 rounded-full" />
                    <span className="text-[10px] font-bold">{c}</span>
                  </div>
                ))}
              </div>

              <ul className="space-y-2 text-sm text-white/90">
                <li className="flex items-center gap-2">
                  <i className="bi bi-check-circle-fill" />
                  <span>{t('auth.layout.bulletMulti', { n: COINS.length })}</span>
                </li>
                <li className="flex items-center gap-2">
                  <i className="bi bi-check-circle-fill" />
                  <span>{t('auth.layout.bulletProducts')}</span>
                </li>
                <li className="flex items-center gap-2">
                  <i className="bi bi-check-circle-fill" />
                  <span>{t('auth.layout.bulletApi')}</span>
                </li>
              </ul>

              <div className="mt-8 flex items-center gap-3 border-t border-white/20 pt-4 text-xs text-white/80">
                <i className="bi bi-shield-lock-fill" />
                {t('auth.layout.securityStrip')}
              </div>
            </div>

            {COIN_CONFIG.BTC && (
              <img
                src={coinLogo('BTC')}
                alt=""
                className="pointer-events-none absolute -bottom-6 -right-6 h-40 w-40 opacity-20"
              />
            )}
          </div>
        </motion.aside>

        <motion.section
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.05 }}
          className="w-full"
        >
          <div className="mx-auto max-w-md">
            <Link to="/welcome" className="mb-6 flex items-center justify-center gap-2.5 lg:hidden">
              <img
                src="/logo.png"
                alt="SatsPay"
                className="h-10 w-10 object-contain drop-shadow-md shrink-0"
              />
              <span className="text-2xl font-bold tracking-tight">SatsPay</span>
            </Link>

            <div className="card p-6 md:p-8">
              <h1 className="mb-1 text-2xl font-bold">{title}</h1>
              {subtitle && <p className="mb-6 text-sm text-ink-muted">{subtitle}</p>}
              {children}
            </div>
            {footer && <div className="mt-4 text-center text-sm text-ink-muted">{footer}</div>}
          </div>
        </motion.section>
      </div>
    </div>
  );
}

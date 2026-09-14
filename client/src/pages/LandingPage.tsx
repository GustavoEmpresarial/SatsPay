import { useEffect, useState } from 'react';
import { Link, Navigate } from 'react-router-dom';
import { motion, useReducedMotion, type Variants } from 'framer-motion';
import { Trans, useTranslation } from 'react-i18next';
import { useAuthStore } from '../stores/auth.js';
import { LanguageSwitch } from '../components/LanguageSwitch.js';
import { ThemeToggle } from '../components/ThemeToggle.js';
import { SiteFooter } from '../components/SiteFooter.js';
import { CookieConsent } from '../components/CookieConsent.js';
import { COIN_CONFIG, COINS } from '@/shared';
import { coinLogo } from '../lib/coinAssets.js';

const fadeUp: Variants = {
  hidden: { opacity: 0, y: 24 },
  show: { opacity: 1, y: 0, transition: { duration: 0.6, ease: 'easeOut' } },
};

const stagger: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.08 } },
};

export function LandingPage() {
  const { t } = useTranslation();
  const user = useAuthStore((s) => s.user);
  const [scrolled, setScrolled] = useState(false);
  const prefersReduced = useReducedMotion();

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  if (user) return <Navigate to="/dashboard" replace />;

  const featureItems = [
    { icon: 'bi-arrow-down-circle-fill', key: 'deposit' },
    { icon: 'bi-arrow-up-circle-fill', key: 'withdraw' },
    { icon: 'bi-globe2', key: 'multi' },
    { icon: 'bi-send-fill', key: 'send' },
    { icon: 'bi-pie-chart-fill', key: 'clarity' },
    { icon: 'bi-clock-history', key: 'history' },
  ] as const;

  const faqItems = t('landing.faq.items', { returnObjects: true }) as Array<{ q: string; a: string }>;

  return (
    <div className="min-h-screen overflow-x-hidden bg-canvas text-ink antialiased selection:bg-bitcoin/30">
      {/* NAV */}
      <header
        className={`sticky top-0 z-40 transition-all ${
          scrolled ? 'border-b border-border bg-canvas/80 backdrop-blur-xl' : 'bg-transparent'
        }`}
      >
        <div className="mx-auto flex max-w-6xl items-center justify-between px-4 py-3 md:px-6 md:py-4">
          <Link to="/welcome" className="flex items-center gap-3">
            <img
              src="/logo.png"
              alt="SatsPay"
              className="h-9 w-9 object-contain drop-shadow-sm shrink-0"
            />
            <span className="text-xl font-bold tracking-tight text-ink">SatsPay</span>
          </Link>

          <div className="flex items-center gap-2 sm:gap-3">
            <ThemeToggle />
            <LanguageSwitch />
            <Link
              to="/login"
              className="rounded-xl px-3.5 py-2 text-sm font-semibold text-ink-muted transition-colors hover:text-ink hover:bg-surface"
            >
              {t('common.signIn')}
            </Link>
            <Link
              to="/register"
              className="inline-flex items-center gap-2 rounded-xl bg-bitcoin px-4 py-2 text-sm font-semibold text-white shadow-md shadow-bitcoin/25 transition-all hover:bg-bitcoin-dark hover:scale-105 active:scale-95"
            >
              {t('common.getStartedFree')} <i className="bi bi-arrow-right text-xs" />
            </Link>
          </div>
        </div>
      </header>

      {/* HERO */}
      <section className="relative overflow-hidden">
        {/* Grid overlay */}
        <div
          className="absolute inset-0 -z-10 opacity-[0.05]"
          style={{
            backgroundImage:
              'linear-gradient(#F7931A 1px, transparent 1px), linear-gradient(90deg, #F7931A 1px, transparent 1px)',
            backgroundSize: '48px 48px',
            maskImage: 'radial-gradient(ellipse at 50% 30%, black 40%, transparent 80%)',
          }}
        />
        {/* Radial glow */}
        <div className="pointer-events-none absolute -top-40 left-1/2 -z-10 h-[600px] w-[900px] -translate-x-1/2 rounded-full bg-bitcoin/10 blur-[120px]" />

        <div className="mx-auto grid max-w-6xl items-center gap-12 px-4 py-16 md:grid-cols-2 md:px-6 md:py-24 lg:py-32">
          <motion.div initial="hidden" animate="show" variants={stagger}>
            <motion.span
              variants={fadeUp}
              className="mb-5 inline-flex items-center gap-2 rounded-full border border-bitcoin/30 bg-bitcoin/10 px-3 py-1 text-xs font-semibold text-bitcoin-dark"
            >
              <span className="relative flex h-2 w-2">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-bitcoin opacity-75" />
                <span className="relative inline-flex h-2 w-2 rounded-full bg-bitcoin" />
              </span>
              {t('landing.hero.badge')}
            </motion.span>

            <motion.h1
              variants={fadeUp}
              className="mb-5 text-4xl font-bold leading-[1.05] tracking-tight md:text-5xl lg:text-6xl"
            >
              {t('landing.hero.title1')}{' '}
              <span className="text-gradient">{t('landing.hero.titleAccent')}</span>
              <br />
              {t('landing.hero.title2')}
            </motion.h1>

            <motion.p variants={fadeUp} className="mb-8 max-w-lg text-lg text-ink-muted md:text-xl">
              <Trans
                i18nKey="landing.hero.subtitle"
                components={{ b: <b className="text-ink" /> }}
              />
            </motion.p>

            <motion.div variants={fadeUp} className="mb-8 flex flex-col gap-3 sm:flex-row">
              <Link
                to="/register"
                className="group inline-flex items-center justify-center gap-2 rounded-lg bg-bitcoin px-6 py-3 text-base font-semibold text-white shadow-xl shadow-bitcoin/30 transition-all hover:bg-bitcoin-dark hover:shadow-bitcoin/50"
              >
                <i className="bi bi-rocket-takeoff" />
                {t('landing.hero.ctaPrimary')}
                <i className="bi bi-arrow-right transition-transform group-hover:translate-x-1" />
              </Link>
              <Link
                to="/login"
                className="inline-flex items-center justify-center gap-2 rounded-lg border border-border bg-surface px-6 py-3 text-base font-medium text-ink transition-colors hover:bg-paper"
              >
                <i className="bi bi-box-arrow-in-right" />
                {t('landing.hero.ctaSecondary')}
              </Link>
            </motion.div>

            <motion.div variants={fadeUp} className="flex flex-wrap items-center gap-x-6 gap-y-2 text-sm text-ink-muted">
              <span className="flex items-center gap-1.5"><i className="bi bi-check-circle-fill text-bitcoin" /> {t('landing.hero.bulletFree')}</span>
              <span className="flex items-center gap-1.5"><i className="bi bi-arrow-left-right text-bitcoin" /> {t('landing.hero.bulletFast')}</span>
              <span className="flex items-center gap-1.5"><i className="bi bi-wallet2 text-bitcoin" /> {t('landing.hero.bulletMulti')}</span>
            </motion.div>
          </motion.div>

          {/* HERO MOCKUP */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.2, ease: 'easeOut' }}
            className="relative"
          >
            <div className="absolute -inset-6 -z-10 rounded-[2rem] bg-gradient-to-br from-bitcoin/20 via-bitcoin/10 to-transparent blur-3xl" />

            <div
              className={`relative rounded-2xl border border-border bg-paper p-6 shadow-card ${
                prefersReduced ? '' : 'animate-float'
              }`}
            >
              <div className="mb-5 flex items-center justify-between">
                <div>
                  <div className="text-xs uppercase tracking-wider text-ink-muted">
                    {t('landing.hero.mockTotalBalance')}
                  </div>
                  <div className="mt-1 font-mono text-3xl font-bold text-ink">
                    0.00214 <span className="text-base font-medium text-ink-muted">BTC</span>
                  </div>
                </div>
                <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-surface/80 p-2 shadow-md border border-border/80">
                  <img src="/logo.png" alt="SatsPay" className="h-full w-full object-contain" />
                </div>
              </div>

              <div className="mb-4 grid grid-cols-3 gap-2">
                {[
                  ['bi-arrow-down-circle-fill', t('landing.hero.mockDeposit')],
                  ['bi-arrow-up-circle-fill', t('landing.hero.mockWithdraw')],
                  ['bi-send-fill', t('landing.hero.mockSend')],
                ].map(([icon, label]) => (
                  <div
                    key={label}
                    className="flex flex-col items-center gap-1.5 rounded-xl border border-border bg-surface/70 py-2.5 text-xs font-semibold text-ink shadow-2xs hover:bg-surface transition-colors"
                  >
                    <i className={`bi ${icon} text-base text-bitcoin`} /> {label}
                  </div>
                ))}
              </div>

              <div className="space-y-2">
                {(['BTC', 'SOL', 'POL', 'USDT', 'LTC', 'DOGE'] as const).map((c) => {
                  const cfg = COIN_CONFIG[c];
                  const mock: Record<string, string> = {
                    BTC: '0.00214',
                    SOL: '1.450',
                    POL: '48.20',
                    USDT: '150.00',
                    LTC: '0.421',
                    DOGE: '10 421',
                  };
                  return (
                    <motion.div
                      key={c}
                      whileHover={{ x: 4 }}
                      className="flex items-center justify-between rounded-xl border border-border bg-surface/70 px-3.5 py-2.5 transition-colors hover:border-bitcoin/40 hover:bg-surface"
                    >
                      <div className="flex items-center gap-3">
                        <img
                          src={coinLogo(c)}
                          alt={cfg.name}
                          className="h-8 w-8 rounded-full object-contain shrink-0 shadow-xs"
                        />
                        <div>
                          <div className="text-xs font-bold text-ink">{cfg.name}</div>
                          <div className="text-[10px] uppercase font-semibold tracking-wider text-bitcoin-dark">{cfg.symbol}</div>
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="font-mono text-xs font-bold text-ink">{mock[c]}</div>
                        <div className="text-[10px] font-semibold text-emerald-600">+{c === 'DOGE' ? '250' : c === 'USDT' ? '10.00' : '0.0001'}</div>
                      </div>
                    </motion.div>
                  );
                })}
              </div>
            </div>

            {/* Floating badges */}
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.6 }}
              className="absolute -left-6 top-8 hidden rounded-2xl border border-border/90 bg-paper/95 p-3.5 shadow-2xl backdrop-blur-md z-20 md:block"
            >
              <div className="flex items-center gap-3">
                <img src={coinLogo('BTC')} alt="BTC" className="h-8 w-8 rounded-full shadow-xs object-contain" />
                <div>
                  <div className="text-[10px] uppercase font-bold text-emerald-600 flex items-center gap-1">
                    <i className="bi bi-check-circle-fill text-xs" />
                    <span>{t('landing.hero.badgeDepositConfirmed')}</span>
                  </div>
                  <div className="text-sm font-bold text-ink font-mono">+0.0001 BTC</div>
                </div>
              </div>
            </motion.div>

            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.9 }}
              className="absolute -right-4 bottom-8 hidden rounded-2xl border border-border/90 bg-paper/95 p-3.5 shadow-2xl backdrop-blur-md z-20 md:block"
            >
              <div className="flex items-center gap-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-bitcoin/15 text-bitcoin-dark">
                  <i className="bi bi-wallet2 text-base" />
                </div>
                <div>
                  <div className="text-[10px] uppercase font-bold text-bitcoin-dark">{t('landing.hero.badgeAccountReady')}</div>
                  <div className="text-xs font-bold text-ink">{t('landing.hero.badgeSendOk')}</div>
                </div>
              </div>
            </motion.div>
          </motion.div>
        </div>
      </section>

      {/* FEATURES */}
      <section id="features" className="py-20 md:py-28">
        <div className="mx-auto max-w-6xl px-4 md:px-6">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            className="mx-auto mb-14 max-w-2xl text-center"
          >
            <span className="mb-3 inline-block rounded-full bg-bitcoin/15 px-3 py-1 text-xs font-semibold uppercase tracking-wider text-bitcoin-dark">
              {t('landing.features.kicker')}
            </span>
            <h2 className="mb-3 text-3xl font-bold text-ink md:text-4xl">{t('landing.features.title')}</h2>
            <p className="text-ink-muted">{t('landing.features.subtitle')}</p>
          </motion.div>

          <motion.div
            initial="hidden"
            whileInView="show"
            viewport={{ once: true, margin: '-50px' }}
            variants={stagger}
            className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3"
          >
            {featureItems.map((f) => (
              <motion.div
                key={f.key}
                variants={fadeUp}
                whileHover={{ y: -4 }}
                className="group relative overflow-hidden rounded-xl border border-border bg-paper p-6 transition-all hover:border-bitcoin/40 hover:shadow-xl hover:shadow-bitcoin/10"
              >
                <div className="pointer-events-none absolute -right-8 -top-8 h-28 w-28 rounded-full bg-bitcoin/10 blur-2xl transition-transform group-hover:scale-150" />
                <div className="relative">
                  <div className="mb-4 inline-flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-bitcoin to-bitcoin-dark text-xl text-white shadow-lg shadow-bitcoin/40">
                    <i className={`bi ${f.icon}`} />
                  </div>
                  <h3 className="mb-2 text-lg font-semibold text-ink">{t(`landing.features.${f.key}.title`)}</h3>
                  <p className="text-sm leading-relaxed text-ink-muted">{t(`landing.features.${f.key}.desc`)}</p>
                </div>
              </motion.div>
            ))}
          </motion.div>
        </div>
      </section>

      {/* HOW IT WORKS */}
      <section id="how" className="border-y border-border bg-surface py-16 md:py-20">
        <div className="mx-auto max-w-6xl px-4 md:px-6">
          <div className="mx-auto mb-12 max-w-2xl text-center">
            <span className="mb-3 inline-block rounded-full bg-bitcoin/15 px-3 py-1 text-xs font-semibold uppercase tracking-wider text-bitcoin-dark">
              {t('landing.how.kicker')}
            </span>
            <h2 className="mb-3 text-3xl font-bold text-ink md:text-4xl">{t('landing.how.title')}</h2>
            <p className="text-ink-muted">{t('landing.how.subtitle')}</p>
          </div>
          <div className="grid gap-6 md:grid-cols-3">
            {[
              { n: '1', title: t('landing.how.s1Title'), desc: t('landing.how.s1Desc') },
              { n: '2', title: t('landing.how.s2Title'), desc: t('landing.how.s2Desc') },
              { n: '3', title: t('landing.how.s3Title'), desc: t('landing.how.s3Desc') },
            ].map((step, i) => (
              <motion.div
                key={step.n}
                initial={{ opacity: 0, y: 16 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ delay: i * 0.08 }}
                className="rounded-2xl border border-border bg-paper p-6"
              >
                <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-xl bg-bitcoin text-lg font-black text-white">
                  {step.n}
                </div>
                <h3 className="mb-2 text-lg font-semibold text-ink">{step.title}</h3>
                <p className="text-sm text-ink-muted">{step.desc}</p>
              </motion.div>
            ))}
          </div>
        </div>
      </section>

      {/* COINS */}
      <section id="coins" className="py-16 md:py-20">
        <div className="mx-auto max-w-6xl px-4 md:px-6">
          <div className="mb-10 text-center">
            <h3 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">{t('landing.coins.kicker')}</h3>
            <p className="mt-2 text-2xl font-bold text-ink">{t('landing.coins.title')}</p>
          </div>

          <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
            {COINS.map((c, i) => {
              const cfg = COIN_CONFIG[c];
              return (
                <motion.div
                  key={c}
                  initial={{ opacity: 0, y: 20 }}
                  whileInView={{ opacity: 1, y: 0 }}
                  viewport={{ once: true }}
                  transition={{ delay: i * 0.1 }}
                  whileHover={{ y: -6, scale: 1.02 }}
                  className="flex flex-col items-center gap-3 rounded-xl border border-border bg-paper p-6 text-center"
                >
                  <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-surface/80 p-2 shadow-sm border border-border/70 group-hover:scale-105 transition-transform">
                    <img
                      src={coinLogo(c)}
                      alt={cfg.name}
                      className="h-full w-full rounded-full object-contain"
                    />
                  </div>
                  <div>
                    <div className="text-lg font-bold text-ink">{cfg.name}</div>
                    <div className="text-xs uppercase tracking-wider font-semibold text-bitcoin-dark">{cfg.symbol}</div>
                  </div>
                  <div className="flex items-center gap-1 text-xs text-ink-muted">
                    <i className="bi bi-check2-circle text-emerald-600" />
                    {t('landing.coins.trio')}
                  </div>
                </motion.div>
              );
            })}
          </div>

          <p className="mt-10 text-center text-sm text-ink-muted">
            {t('landing.devsNote.text')}{' '}
            <Link to="/documentation" className="font-semibold text-bitcoin-dark underline-offset-2 hover:underline">
              {t('landing.devsNote.link')}
            </Link>
          </p>
        </div>
      </section>

      {/* FAQ */}
      <section id="faq" className="border-t border-border bg-surface py-20">
        <div className="mx-auto max-w-3xl px-4 md:px-6">
          <div className="mb-10 text-center">
            <span className="mb-3 inline-block rounded-full bg-bitcoin/15 px-3 py-1 text-xs font-semibold uppercase tracking-wider text-bitcoin-dark">
              {t('landing.faq.kicker')}
            </span>
            <h2 className="text-3xl font-bold text-ink md:text-4xl">{t('landing.faq.title')}</h2>
          </div>

          <div className="space-y-3">
            {faqItems.map((item, i) => (
              <motion.details
                key={item.q}
                initial={{ opacity: 0, y: 10 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ delay: i * 0.05 }}
                className="group overflow-hidden rounded-xl border border-border bg-paper"
              >
                <summary className="flex cursor-pointer list-none items-center justify-between px-5 py-4 font-semibold text-ink">
                  <span className="flex items-center gap-3">
                    <i className="bi bi-patch-question-fill text-bitcoin-dark" />
                    {item.q}
                  </span>
                  <i className="bi bi-chevron-down transition-transform group-open:rotate-180" />
                </summary>
                <div className="px-5 pb-5 text-sm text-ink-muted">{item.a}</div>
              </motion.details>
            ))}
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="relative overflow-hidden py-20 md:py-24 bg-gradient-to-br from-bitcoin via-bitcoin-dark to-[#8A4E08] text-white">
        <div
          className="pointer-events-none absolute inset-0 opacity-10"
          style={{
            backgroundImage: 'radial-gradient(circle at 1px 1px, white 1px, transparent 0)',
            backgroundSize: '24px 24px',
          }}
        />

        <motion.div
          initial={{ opacity: 0, y: 30 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          className="relative z-10 mx-auto max-w-3xl px-4 text-center text-white md:px-6"
        >
          <div className="mb-6 inline-flex h-16 w-16 items-center justify-center rounded-2xl bg-white/20 text-3xl text-white backdrop-blur-sm shadow-inner">
            <i className="bi bi-rocket-takeoff-fill" />
          </div>
          <h2 className="mb-4 text-3xl font-bold text-white md:text-5xl">{t('landing.cta.title')}</h2>
          <p className="mx-auto mb-8 max-w-xl text-lg text-white/95 leading-relaxed">{t('landing.cta.subtitle')}</p>
          <div className="flex flex-col justify-center gap-3 sm:flex-row">
            <Link
              to="/register"
              className="inline-flex items-center justify-center gap-2 rounded-xl bg-white px-8 py-3.5 text-base font-bold text-bitcoin-dark shadow-xl transition-all hover:scale-105 hover:bg-white/95"
            >
              <i className="bi bi-person-plus-fill" /> {t('landing.cta.primary')}
            </Link>
            <Link
              to="/login"
              className="inline-flex items-center justify-center gap-2 rounded-xl border border-white/40 bg-white/10 px-8 py-3.5 text-base font-semibold text-white backdrop-blur-sm transition-all hover:bg-white/20"
            >
              <i className="bi bi-box-arrow-in-right" /> {t('landing.cta.secondary')}
            </Link>
          </div>
        </motion.div>
      </section>

      <SiteFooter />
      <CookieConsent />
    </div>
  );
}

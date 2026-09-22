import { useEffect, useState } from 'react';
import { NavLink, Outlet, useNavigate, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../stores/auth.js';
import { usePrefsStore } from '../stores/prefs.js';
import { api, bootstrapSession, forceReauth } from '../lib/api.js';
import type { PublicUser } from '@/shared';
import { LanguageSwitch } from './LanguageSwitch.js';
import { ThemeToggle } from './ThemeToggle.js';
import { clsx } from 'clsx';

export function AppLayout() {
  const { t } = useTranslation();
  const { user, logout, updateUser } = useAuthStore();
  const { mode, setMode } = usePrefsStore();
  const navigate = useNavigate();
  const location = useLocation();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  const isMerchantRoute =
    location.pathname.startsWith('/merchant') ||
    location.pathname === '/developer-wallets' ||
    location.pathname.startsWith('/developer') ||
    location.pathname.startsWith('/oauth/apps') ||
    location.pathname === '/api-keys';

  const isPersonalRoute =
    location.pathname === '/dashboard' ||
    location.pathname.startsWith('/wallets') ||
    location.pathname === '/faucet' ||
    location.pathname === '/faucetlist' ||
    location.pathname === '/swap' ||
    location.pathname === '/stake' ||
    location.pathname === '/lend' ||
    location.pathname === '/analytics' ||
    location.pathname === '/settings' ||
    location.pathname === '/support';

  // Sincroniza o modo de conta com a rota atual automaticamente
  useEffect(() => {
    if (isMerchantRoute && mode !== 'merchant') {
      setMode('merchant');
    } else if (isPersonalRoute && mode !== 'user') {
      setMode('user');
    }
  }, [location.pathname, isMerchantRoute, isPersonalRoute, mode, setMode]);

  // After zustand rehydrates identity, restore access JWT from HttpOnly cookie then sync /auth/me.
  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      const currentUser = useAuthStore.getState().user;
      if (!currentUser) return;
      if (!useAuthStore.getState().accessToken) {
        const ok = await bootstrapSession();
        if (!ok || cancelled) return;
      }
      try {
        const r = await api<{ user: PublicUser }>('/auth/me');
        if (!cancelled && r?.user) updateUser(r.user);
      } catch (err) {
        // Dead session → login. Ignore only true network/deploy blips.
        const status = typeof err === 'object' && err && 'status' in err ? Number((err as { status: number }).status) : 0;
        if (status === 401 || status === 403) {
          forceReauth();
        }
      }
    };
    const unsub = useAuthStore.persist.onFinishHydration(() => {
      void run();
    });
    if (useAuthStore.persist.hasHydrated()) void run();
    return () => {
      cancelled = true;
      unsub();
    };
  }, [updateUser]);

  useEffect(() => {
    setMobileMenuOpen(false);
  }, [location.pathname]);

  const personalNav = [
    { to: '/dashboard', label: t('nav.dashboard'), icon: 'bi-speedometer2' },
    { to: '/wallets', label: t('nav.wallets'), icon: 'bi-wallet2' },
    { to: '/faucet', label: t('nav.faucet'), icon: 'bi-droplet-fill' },
    { to: '/faucetlist', label: t('nav.faucetlist'), icon: 'bi-list-stars' },
    { to: '/swap', label: t('nav.swap'), icon: 'bi-arrow-left-right' },
    { to: '/referrals', label: 'Indicações', icon: 'bi-people-fill', badge: '10% Bônus' },
    { to: '/airdrop', label: 'Airdrop $SATS', icon: 'bi-gift-fill', badge: 'Recompensas' },
    { to: '/stake', label: t('nav.stake'), icon: 'bi-graph-up-arrow', badge: t('nav.comingSoon', 'Em breve') },
    { to: '/lend', label: t('nav.lend'), icon: 'bi-bank', badge: t('nav.maintenance', 'Manutenção') },
    { to: '/analytics', label: t('nav.analytics'), icon: 'bi-bar-chart-line-fill' },
    { to: '/support', label: t('nav.support'), icon: 'bi-life-preserver' },
    { to: '/settings', label: t('nav.settings'), icon: 'bi-gear-fill' },
  ];

  const merchantNav = [
    { to: '/merchant/dashboard', label: 'Dashboard', icon: 'bi-speedometer2' },
    { to: '/developer-wallets', label: t('nav.devWallets'), icon: 'bi-piggy-bank-fill' },
    { to: '/merchant/sites', label: t('nav.faucetSites'), icon: 'bi-shop-window' },
    { to: '/merchant/deposits', label: 'Gateway de Depósitos', icon: 'bi-box-arrow-in-down' },
    { to: '/developer/apps', label: 'Login com SatsPay (OAuth)', icon: 'bi-shield-lock-fill', badge: 'SSO' },
    { to: '/api-keys', label: t('nav.apiKeys'), icon: 'bi-key-fill' },
    { to: '/docs', label: t('nav.apiDocs'), icon: 'bi-code-slash' },
    { to: '/support', label: t('nav.support'), icon: 'bi-life-preserver' },
  ];

  const infraAndDocsNav = [
    { to: '/documentation', label: 'Guia da Plataforma', icon: 'bi-journal-text' },
    { to: '/status', label: t('nav.uptime'), icon: 'bi-activity', badge: t('nav.live') },
  ];

  const mobileBottomNav =
    mode === 'merchant'
      ? [
          { to: '/merchant/dashboard', label: 'Dashboard', icon: 'bi-speedometer2' },
          { to: '/developer-wallets', label: 'Saldo', icon: 'bi-piggy-bank-fill' },
          { to: '/merchant/sites', label: 'Sites', icon: 'bi-shop-window' },
          { to: '/merchant/deposits', label: 'Gateway', icon: 'bi-box-arrow-in-down' },
        ]
      : [
          { to: '/dashboard', label: t('nav.dashboard'), icon: 'bi-speedometer2' },
          { to: '/wallets', label: t('nav.wallets'), icon: 'bi-wallet2' },
          { to: '/faucet', label: t('nav.faucet'), icon: 'bi-droplet-fill' },
          { to: '/swap', label: t('nav.swap'), icon: 'bi-arrow-left-right' },
        ];

  const handleLogout = async () => {
    // Revoke the refresh token server-side and clear the HttpOnly cookie.
    await api('/auth/logout', { method: 'POST', skipAuth: true }).catch(() => {});
    logout();
    navigate('/login');
  };

  const handleModeSwitch = (newMode: 'user' | 'merchant') => {
    if (newMode === 'merchant') {
      setMode('merchant');
      navigate('/merchant/dashboard');
    } else {
      setMode('user');
      navigate('/dashboard');
    }
  };

  const username = user?.username?.trim() || '';
  const emailLocal = user?.email?.split('@')[0] ?? '';
  const displayName = username || emailLocal;
  const initial = (displayName[0] ?? '?').toUpperCase();

  const renderItem = (item: { to: string; label: string; icon: string; badge?: string }) => (
    <NavLink
      key={item.to}
      to={item.to}
      end={item.to === '/'}
      onClick={() => setMobileMenuOpen(false)}
      className={({ isActive }) =>
        clsx(
          'group flex items-center gap-3 rounded-xl px-3.5 py-2.5 text-sm font-medium transition-all duration-150',
          isActive
            ? mode === 'merchant'
              ? 'bg-emerald-600 text-white font-semibold shadow-sm shadow-emerald-600/30'
              : 'bg-bitcoin text-white font-semibold shadow-sm shadow-bitcoin/30'
            : 'text-ink-muted hover:text-ink hover:bg-surface',
        )
      }
    >
      {({ isActive }) => (
        <>
          <i
            className={clsx(
              `bi ${item.icon} text-base`,
              isActive ? 'text-white' : 'text-ink-muted group-hover:text-ink',
            )}
          />
          <span className="truncate">{item.label}</span>
          {item.badge && (
            <span
              className={clsx(
                'ml-auto flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-bold',
                isActive ? 'bg-white/20 text-white' : 'bg-emerald-500/10 text-emerald-600',
              )}
            >
              <span
                className={clsx(
                  'h-1.5 w-1.5 rounded-full animate-pulse',
                  isActive ? 'bg-white' : 'bg-emerald-500',
                )}
              />
              {item.badge}
            </span>
          )}
        </>
      )}
    </NavLink>
  );

  return (
    <div className="flex min-h-screen bg-surface">
      {/* Sidebar desktop */}
      <aside className="sticky top-0 hidden h-screen w-72 shrink-0 border-r border-border bg-paper lg:flex lg:flex-col justify-between">
        {/* 1. Topo Fixo (Brand + User Card + Account Mode Switcher) */}
        <div className="shrink-0 border-b border-border/70 pb-3.5 bg-paper">
          {/* Brand */}
          <div className="flex items-center gap-3 px-6 pt-5 pb-3">
            <img
              src="/logo.png"
              alt="SatsPay"
              className="h-11 w-11 object-contain drop-shadow-md shrink-0 transition-transform hover:scale-105"
            />
            <div>
              <div className="text-xl font-extrabold leading-tight tracking-tight text-ink">SatsPay</div>
              <div className="text-[10px] font-semibold uppercase tracking-widest text-ink-muted">
                {mode === 'merchant' ? 'Painel Comerciante' : t('common.cryptoPlatform')}
              </div>
            </div>
          </div>

          {/* User card */}
          <div className="mx-4 rounded-xl border border-border/80 bg-surface/70 p-3">
            <div className="flex items-center gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-bitcoin/15 text-sm font-bold text-bitcoin-dark">
                {initial}
              </div>
              <div className="min-w-0 flex-1">
                <div className="truncate text-xs font-semibold text-ink">
                  {username ? `@${username}` : displayName}
                </div>
                {user?.email ? (
                  <div className="truncate text-[10px] text-ink-muted">{user.email}</div>
                ) : null}
                <div className="mt-1 flex items-center gap-1.5 text-[10px]">
                  {user?.twoFactorEnabled ? (
                    <span className="inline-flex items-center gap-1 font-semibold text-emerald-600">
                      <i className="bi bi-shield-fill-check text-xs" /> {t('common.twoFactorOn', { defaultValue: '2FA Ativo' })}
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 text-ink-muted font-medium">
                      <i className="bi bi-shield text-xs" /> {t('common.twoFactorOptional', { defaultValue: '2FA Opcional' })}
                    </span>
                  )}
                </div>
              </div>
            </div>
          </div>

          {/* Account Mode Switcher Tabs */}
          <div className="mx-4 mt-3 grid grid-cols-2 rounded-xl bg-surface p-1 border border-border/80 text-xs font-bold">
            <button
              type="button"
              onClick={() => handleModeSwitch('user')}
              className={clsx(
                'flex items-center justify-center gap-1.5 rounded-lg py-1.5 transition-all text-xs',
                mode === 'user'
                  ? 'bg-paper text-ink shadow-xs font-bold'
                  : 'text-ink-muted hover:text-ink',
              )}
            >
              <i className="bi bi-person-fill text-xs text-bitcoin" />
              <span>Pessoal</span>
            </button>
            <button
              type="button"
              onClick={() => handleModeSwitch('merchant')}
              className={clsx(
                'flex items-center justify-center gap-1.5 rounded-lg py-1.5 transition-all text-xs',
                mode === 'merchant'
                  ? 'bg-paper text-emerald-700 shadow-xs font-bold'
                  : 'text-ink-muted hover:text-ink',
              )}
            >
              <i className="bi bi-shop text-xs text-emerald-600" />
              <span>Comerciante</span>
            </button>
          </div>
        </div>

        {/* 2. Navegação Rolável */}
        <nav className="flex-1 overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden space-y-1 px-3.5 py-3">
          {mode === 'merchant' ? (
            <div className="space-y-1">
              <div className="px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider text-emerald-700/80">
                Suíte do Comerciante
              </div>
              {merchantNav.map(renderItem)}

              <div className="pt-4">
                <div className="rounded-xl border border-emerald-500/25 bg-emerald-500/10 p-3 text-xs space-y-1.5">
                  <div className="flex items-center gap-1.5 font-bold text-emerald-800">
                    <i className="bi bi-shield-check" />
                    <span>Modo Comerciante</span>
                  </div>
                  <p className="text-[11px] text-emerald-900/70 leading-snug">
                    Gerencie gateways de pagamento, APIs e sites parceiros.
                  </p>
                  <button
                    type="button"
                    onClick={() => handleModeSwitch('user')}
                    className="mt-1 w-full rounded-lg bg-paper border border-border py-1.5 text-center text-[11px] font-bold text-ink hover:bg-surface transition-all"
                  >
                    ← Voltar ao Modo Pessoal
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <div className="space-y-1">
              <div className="px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider text-ink-muted">
                Menu Pessoal
              </div>
              {personalNav.map(renderItem)}

              <div className="pt-3 border-t border-border/60 mt-3 space-y-1">
                <div className="px-3 py-1 text-[10px] font-bold uppercase tracking-wider text-ink-muted">
                  Recursos
                </div>
                {infraAndDocsNav.map(renderItem)}
              </div>
            </div>
          )}
        </nav>

        {/* 3. Rodapé Fixo (Idioma, Sair e Copyright) */}
        <div className="shrink-0 space-y-2.5 border-t border-border/70 p-3.5 bg-paper">
          <div className="flex items-center justify-between px-1 gap-2">
            <span className="text-xs font-medium text-ink-muted">{t('common.theme')}</span>
            <ThemeToggle />
          </div>
          <div className="flex items-center justify-between px-1 gap-2">
            <span className="text-xs font-medium text-ink-muted">{t('common.language')}</span>
            <LanguageSwitch />
          </div>
          <button className="btn-secondary w-full text-xs py-2" onClick={handleLogout}>
            <i className="bi bi-box-arrow-right mr-1.5" />
            {t('common.signOut')}
          </button>
          <div className="pt-0.5 text-center text-[10px] text-ink-muted">
            SatsPay © {new Date().getFullYear()}
          </div>
        </div>
      </aside>

      {/* Mobile Drawer (Menu Completo no Mobile) */}
      {mobileMenuOpen && (
        <div className="fixed inset-0 z-[100] lg:hidden">
          <div
            className="fixed inset-0 bg-black/60 backdrop-blur-sm transition-opacity"
            onClick={() => setMobileMenuOpen(false)}
          />
          <aside className="fixed inset-y-0 left-0 z-[101] flex w-72 flex-col justify-between border-r border-border bg-paper shadow-2xl">
            {/* Topo Fixo Mobile */}
            <div className="shrink-0 border-b border-border/70 pb-3.5 bg-paper">
              <div className="flex items-center justify-between px-5 pt-4 pb-2">
                <div className="flex items-center gap-3">
                  <img
                    src="/logo.png"
                    alt="SatsPay"
                    className="h-10 w-10 object-contain drop-shadow-md shrink-0"
                  />
                  <div>
                    <div className="text-lg font-bold leading-tight tracking-tight text-ink">SatsPay</div>
                    <div className="text-[10px] font-semibold uppercase tracking-widest text-ink-muted">
                      {mode === 'merchant' ? 'Painel Comerciante' : t('common.cryptoPlatform')}
                    </div>
                  </div>
                </div>
                <button
                  onClick={() => setMobileMenuOpen(false)}
                  className="rounded-lg p-1.5 text-ink-muted hover:bg-surface hover:text-ink"
                >
                  <i className="bi bi-x-lg text-lg" />
                </button>
              </div>

              {/* User card mobile */}
              <div className="mx-3.5 mt-2 rounded-xl border border-border/80 bg-surface/70 p-2.5">
                <div className="flex items-center gap-2.5">
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-bitcoin/15 text-xs font-bold text-bitcoin-dark">
                    {initial}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-xs font-semibold text-ink">
                      {username ? `@${username}` : displayName}
                    </div>
                    {user?.email ? (
                      <div className="truncate text-[10px] text-ink-muted">{user.email}</div>
                    ) : null}
                    <div className="mt-0.5 flex items-center gap-1 text-[10px]">
                      {user?.twoFactorEnabled ? (
                        <span className="inline-flex items-center gap-1 font-semibold text-emerald-600">
                          <i className="bi bi-shield-fill-check text-xs" /> {t('common.twoFactorOn', { defaultValue: '2FA Ativo' })}
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 text-ink-muted font-medium">
                          <i className="bi bi-shield text-xs" /> {t('common.twoFactorOptional', { defaultValue: '2FA Opcional' })}
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              </div>

              {/* Account Mode Switcher Tabs Mobile */}
              <div className="mx-3.5 mt-2.5 grid grid-cols-2 rounded-xl bg-surface p-1 border border-border/80 text-xs font-bold">
                <button
                  type="button"
                  onClick={() => handleModeSwitch('user')}
                  className={clsx(
                    'flex items-center justify-center gap-1.5 rounded-lg py-1.5 transition-all text-xs',
                    mode === 'user'
                      ? 'bg-paper text-ink shadow-xs font-bold'
                      : 'text-ink-muted hover:text-ink',
                  )}
                >
                  <i className="bi bi-person-fill text-xs text-bitcoin" />
                  <span>Pessoal</span>
                </button>
                <button
                  type="button"
                  onClick={() => handleModeSwitch('merchant')}
                  className={clsx(
                    'flex items-center justify-center gap-1.5 rounded-lg py-1.5 transition-all text-xs',
                    mode === 'merchant'
                  ? 'bg-paper text-emerald-700 shadow-xs font-bold'
                      : 'text-ink-muted hover:text-ink',
                  )}
                >
                  <i className="bi bi-shop text-xs text-emerald-600" />
                  <span>Comerciante</span>
                </button>
              </div>
            </div>

            {/* Navegação Mobile */}
            <nav className="flex-1 overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden space-y-1 px-3 py-2.5">
              {mode === 'merchant' ? (
                <div className="space-y-1">
                  <div className="px-3 py-1 text-[10px] font-bold uppercase tracking-wider text-emerald-700/80">
                    Suíte do Comerciante
                  </div>
                  {merchantNav.map(renderItem)}
                </div>
              ) : (
                <div className="space-y-1">
                  <div className="px-3 py-1 text-[10px] font-bold uppercase tracking-wider text-ink-muted">
                    Menu Pessoal
                  </div>
                  {personalNav.map(renderItem)}

                  <div className="pt-3 border-t border-border/60 mt-3 space-y-1">
                    <div className="px-3 py-1 text-[10px] font-bold uppercase tracking-wider text-ink-muted">
                      Recursos
                    </div>
                    {infraAndDocsNav.map(renderItem)}
                  </div>
                </div>
              )}
            </nav>

            {/* Rodapé Fixo Mobile */}
            <div className="shrink-0 space-y-2 border-t border-border/70 p-3 bg-paper">
              <div className="flex items-center justify-between px-1 gap-2">
                <span className="text-xs font-medium text-ink-muted">{t('common.theme')}</span>
                <ThemeToggle />
              </div>
              <div className="flex items-center justify-between px-1 gap-2">
                <span className="text-xs font-medium text-ink-muted">{t('common.language')}</span>
                <LanguageSwitch />
              </div>
              <button className="btn-secondary w-full text-xs py-2" onClick={handleLogout}>
                <i className="bi bi-box-arrow-right mr-1.5" />
                {t('common.signOut')}
              </button>
            </div>
          </aside>
        </div>
      )}

      {/* Mobile top bar */}
      <div className="fixed inset-x-0 top-0 z-30 flex h-14 items-center justify-between border-b border-border bg-paper/90 px-4 backdrop-blur-md lg:hidden">
        <div className="flex items-center gap-2.5">
          <button
            onClick={() => setMobileMenuOpen(true)}
            className="flex h-9 w-9 items-center justify-center rounded-lg border border-border bg-surface text-ink hover:bg-paper active:scale-95 transition-all"
            aria-label="Abrir Menu"
          >
            <i className="bi bi-list text-xl" />
          </button>
          <div className="flex items-center gap-2.5">
            <img
              src="/logo.png"
              alt="SatsPay"
              className="h-8 w-8 object-contain drop-shadow-sm shrink-0"
            />
            <span className="font-extrabold text-base tracking-tight text-ink">SatsPay</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {displayName ? (
            <span className="max-w-[7.5rem] truncate text-xs font-medium text-ink-muted">
              {username ? `@${username}` : displayName}
            </span>
          ) : null}
          <ThemeToggle />
          <LanguageSwitch />
          <button
            className="rounded-lg p-2 text-ink-muted hover:bg-surface"
            onClick={handleLogout}
            title={t('common.signOut')}
          >
            <i className="bi bi-box-arrow-right text-lg" />
          </button>
        </div>
      </div>

      {/* Main content — overflow on inner wrapper so page modals aren't trapped
          under the mobile header/nav stacking context (backdrop stripe bug). */}
      <main className="flex-1 pb-24 pt-14 lg:pb-0 lg:pt-0">
        <div className="mx-auto w-full max-w-7xl overflow-x-clip px-4 py-6 sm:px-6 md:py-8 xl:px-10">
          <Outlet />
        </div>
      </main>

      {/* Mobile bottom nav */}
      <nav className="fixed inset-x-0 bottom-0 z-30 flex items-center justify-around border-t border-border bg-paper/95 pb-[env(safe-area-inset-bottom)] backdrop-blur-md lg:hidden">
        {mobileBottomNav.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            className={({ isActive }) =>
              clsx(
                'flex flex-1 flex-col items-center gap-0.5 py-2.5 text-[10px] font-medium transition-colors',
                isActive ? 'text-bitcoin-dark font-bold' : 'text-ink-muted hover:text-ink',
              )
            }
          >
            <i className={`bi ${item.icon} text-lg`} />
            <span className="truncate">{item.label}</span>
          </NavLink>
        ))}
        {/* Botão de Menu Mais */}
        <button
          type="button"
          onClick={() => setMobileMenuOpen(true)}
          className="flex flex-1 flex-col items-center gap-0.5 py-2.5 text-[10px] font-medium text-ink-muted hover:text-ink transition-colors"
        >
          <i className="bi bi-grid text-lg" />
          <span className="truncate">{t('common.menu')}</span>
        </button>
      </nav>
    </div>
  );
}

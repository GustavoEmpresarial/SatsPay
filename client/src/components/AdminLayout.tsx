import { useState } from 'react';
import { NavLink, Outlet, useNavigate } from 'react-router-dom';
import { useAdminStore } from '../stores/admin.js';
import { useAuthStore } from '../stores/auth.js';
import { api } from '../lib/api.js';
import { clsx } from 'clsx';

/** Admin UI is always pt-BR — ignore user-app language switch. */
const NAV = [
  { to: '/admin', label: 'Visão geral', icon: 'bi-speedometer2' },
  { to: '/admin/users', label: 'Usuários', icon: 'bi-people-fill' },
  { to: '/admin/withdrawals', label: 'Saques', icon: 'bi-hourglass-split' },
  { to: '/admin/stake', label: 'Tesouraria', icon: 'bi-safe2' },
  { to: '/admin/merchants', label: 'Comerciantes', icon: 'bi-shop' },
  { to: '/admin/faucet-sites', label: 'Faucetlist', icon: 'bi-list-stars' },
  { to: '/admin/support', label: 'Suporte', icon: 'bi-headset' },
  { to: '/admin/telemetry', label: 'Telemetria', icon: 'bi-activity' },
] as const;

export function AdminLayout() {
  const { admin, logout } = useAdminStore();
  const user = useAuthStore((s) => s.user);
  const authLogout = useAuthStore((s) => s.logout);
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);

  const displayEmail = admin?.email || (user?.role === 'ADMIN' ? user.email : 'Admin');

  const handleLogout = async () => {
    await api('/auth/admin/logout', { method: 'POST', skipAuth: true }).catch(() => {});
    logout();
    authLogout();
    navigate('/admin/login');
  };

  const links = (
    <>
      {NAV.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.to === '/admin'}
          onClick={() => setOpen(false)}
          className={({ isActive }) =>
            clsx(
              'flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors',
              isActive ? 'bg-rose-500/10 text-rose-600' : 'text-ink-muted hover:bg-surface hover:text-ink',
            )
          }
        >
          <i className={`bi ${item.icon} text-lg`} />
          {item.label}
        </NavLink>
      ))}
    </>
  );

  return (
    <div className="flex min-h-screen bg-canvas text-ink antialiased">
      <aside className="sticky top-0 hidden h-screen w-64 shrink-0 border-r border-border bg-paper lg:flex lg:flex-col">
        <div className="flex items-center gap-2.5 border-b border-border px-6 py-5">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-rose-500 to-rose-700 text-lg text-white shadow-lg shadow-rose-500/40">
            <i className="bi bi-shield-lock-fill" />
          </div>
          <div>
            <div className="text-lg font-bold leading-tight">Admin</div>
            <div className="text-[10px] uppercase tracking-widest text-ink-muted">SatsPay</div>
          </div>
        </div>

        <div className="mx-4 mt-4 rounded-xl border border-border bg-surface p-3">
          <div className="text-[10px] uppercase tracking-widest text-ink-muted">Conectado como</div>
          <div className="truncate text-sm font-semibold">{displayEmail}</div>
        </div>

        <nav className="mt-6 flex-1 space-y-0.5 px-3">{links}</nav>

        <div className="border-t border-border p-4">
          <button
            className="w-full rounded-lg border border-border bg-surface py-2 text-sm font-medium text-ink-muted hover:bg-paper hover:text-ink"
            onClick={handleLogout}
          >
            <i className="bi bi-box-arrow-right mr-1.5" />
            Sair
          </button>
          <a href="/" className="mt-2 block text-center text-[10px] text-ink-muted hover:text-ink">
            ← App do usuário
          </a>
        </div>
      </aside>

      <div className="fixed inset-x-0 top-0 z-30 flex h-14 items-center justify-between border-b border-border bg-canvas/90 px-4 backdrop-blur-md lg:hidden">
        <button type="button" onClick={() => setOpen(true)} className="rounded-lg p-2 text-ink" aria-label="Abrir menu">
          <i className="bi bi-list text-xl" />
        </button>
        <div className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-rose-500 text-white">
            <i className="bi bi-shield-lock-fill" />
          </div>
          <span className="font-semibold">Admin</span>
        </div>
        <button onClick={handleLogout} className="rounded-lg p-2 text-ink-muted hover:text-ink" aria-label="Sair">
          <i className="bi bi-box-arrow-right text-lg" />
        </button>
      </div>

      {open && (
        <div className="fixed inset-0 z-[100] lg:hidden">
          <button type="button" className="absolute inset-0 bg-black/40" aria-label="Fechar menu" onClick={() => setOpen(false)} />
          <div className="absolute inset-y-0 left-0 z-[101] flex w-72 flex-col bg-paper shadow-xl">
            <div className="flex items-center justify-between border-b border-border px-4 py-4">
              <span className="font-bold">Menu</span>
              <button type="button" onClick={() => setOpen(false)} className="p-2 text-ink-muted" aria-label="Fechar">
                <i className="bi bi-x-lg" />
              </button>
            </div>
            <div className="px-3 py-3 text-xs text-ink-muted truncate">{admin?.email}</div>
            <nav className="flex-1 space-y-0.5 px-3">{links}</nav>
          </div>
        </div>
      )}

      <main className="flex-1 overflow-x-hidden pt-14 lg:pt-0">
        <div className="mx-auto w-full max-w-7xl px-4 py-6 md:px-6 md:py-8 xl:px-10">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

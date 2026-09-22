import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { formatAdminDate } from '../lib/admin.js';

interface AdminUserItem {
  id: string;
  email: string;
  username?: string | null;
  role: string;
  merchant_status: string;
  two_factor_enabled: boolean;
  created_at: string;
  last_login_at?: string | null;
  erased_at?: string | null;
}

const ROLE_FILTERS = ['ALL', 'USER', 'ADMIN'] as const;
const ROLE_LABEL: Record<(typeof ROLE_FILTERS)[number], string> = {
  ALL: 'Todos',
  USER: 'Usuários',
  ADMIN: 'Admins',
};

function roleClass(role: string): string {
  return role === 'ADMIN' ? 'bg-rose-500/15 text-rose-800' : 'bg-surface text-ink-muted';
}

function merchantClass(status: string): string {
  switch (status) {
    case 'APPROVED':
      return 'bg-emerald-500/15 text-emerald-800';
    case 'PENDING':
      return 'bg-amber-500/15 text-amber-800';
    case 'SUSPENDED':
    case 'REJECTED':
      return 'bg-rose-500/15 text-rose-800';
    default:
      return 'bg-surface text-ink-muted';
  }
}

export function AdminUsersPage() {
  const [roleFilter, setRoleFilter] = useState<(typeof ROLE_FILTERS)[number]>('ALL');
  const [searchTerm, setSearchTerm] = useState('');
  const [includeErased, setIncludeErased] = useState(false);
  const [debouncedQ, setDebouncedQ] = useState('');

  // Simple debounce via effect-free pattern: apply search on blur/Enter or button.
  const applySearch = () => setDebouncedQ(searchTerm.trim());

  const listQ = useQuery({
    queryKey: ['admin-users', roleFilter, debouncedQ, includeErased],
    queryFn: () => {
      const params = new URLSearchParams();
      if (roleFilter !== 'ALL') params.set('role', roleFilter);
      if (debouncedQ) params.set('q', debouncedQ);
      if (includeErased) params.set('include_erased', 'true');
      params.set('limit', '200');
      return api<{ users: AdminUserItem[] }>(`/admin/users?${params.toString()}`);
    },
    refetchInterval: 30_000,
  });

  const users = listQ.data?.users ?? [];

  const stats = useMemo(() => {
    const total = users.length;
    const admins = users.filter((u) => u.role === 'ADMIN').length;
    const with2fa = users.filter((u) => u.two_factor_enabled).length;
    const merchants = users.filter((u) => u.merchant_status && u.merchant_status !== 'NONE').length;
    const erased = users.filter((u) => !!u.erased_at).length;
    const active24h = users.filter((u) => {
      if (!u.last_login_at) return false;
      const t = Date.parse(u.last_login_at);
      return Number.isFinite(t) && Date.now() - t < 24 * 60 * 60 * 1000;
    }).length;
    return { total, admins, with2fa, merchants, erased, active24h };
  }, [users]);

  return (
    <div className="space-y-8 pb-16">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full border border-rose-500/30 bg-rose-500/10 px-3 py-1 text-xs font-black text-rose-700">
            <i className="bi bi-people-fill" /> Contas da plataforma
          </div>
          <h1 className="mt-1 text-2xl sm:text-3xl font-black text-ink tracking-tight">Usuários</h1>
          <p className="text-xs sm:text-sm text-ink-muted">
            Consulta de contas (e-mail revelado só para admin). HOUSE interno fica fora da lista.
          </p>
        </div>
        <button
          type="button"
          onClick={() => void listQ.refetch()}
          className="inline-flex items-center gap-2 self-start rounded-xl border border-border bg-paper px-4 py-2.5 text-xs font-bold text-ink"
        >
          <i className={`bi bi-arrow-repeat ${listQ.isFetching ? 'animate-spin' : ''}`} />
          Atualizar
        </button>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-5 gap-4">
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="text-[11px] font-bold uppercase tracking-wider text-ink-muted">Nesta lista</div>
          <div className="mt-1 text-2xl font-black">{stats.total}</div>
        </div>
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="text-[11px] font-bold uppercase tracking-wider text-ink-muted">Admins</div>
          <div className="mt-1 text-2xl font-black text-rose-700">{stats.admins}</div>
        </div>
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="text-[11px] font-bold uppercase tracking-wider text-ink-muted">Login 24h</div>
          <div className="mt-1 text-2xl font-black text-emerald-700">{stats.active24h}</div>
        </div>
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="text-[11px] font-bold uppercase tracking-wider text-ink-muted">Com 2FA</div>
          <div className="mt-1 text-2xl font-black">{stats.with2fa}</div>
        </div>
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="text-[11px] font-bold uppercase tracking-wider text-ink-muted">Merchants</div>
          <div className="mt-1 text-2xl font-black">{stats.merchants}</div>
        </div>
      </div>

      <div className="flex flex-col gap-3 sm:flex-row sm:flex-wrap sm:items-center">
        <div className="flex flex-wrap gap-2">
          {ROLE_FILTERS.map((r) => (
            <button
              key={r}
              type="button"
              onClick={() => setRoleFilter(r)}
              className={`rounded-full px-3 py-1.5 text-xs font-bold ${
                roleFilter === r ? 'bg-bitcoin text-white' : 'border border-border text-ink-muted'
              }`}
            >
              {ROLE_LABEL[r]}
            </button>
          ))}
        </div>

        <form
          className="flex flex-1 flex-wrap items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            applySearch();
          }}
        >
          <input
            type="search"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Buscar e-mail, username ou UUID…"
            className="min-w-[12rem] flex-1 rounded-xl border border-border bg-paper px-3 py-2 text-sm"
          />
          <button
            type="submit"
            className="rounded-xl border border-border bg-surface px-3 py-2 text-xs font-bold text-ink"
          >
            Buscar
          </button>
          <label className="inline-flex items-center gap-2 text-xs font-medium text-ink-muted">
            <input
              type="checkbox"
              checked={includeErased}
              onChange={(e) => setIncludeErased(e.target.checked)}
              className="rounded border-border"
            />
            Incluir apagados (LGPD)
            {includeErased && stats.erased > 0 ? (
              <span className="text-rose-700">({stats.erased})</span>
            ) : null}
          </label>
        </form>
      </div>

      <div className="overflow-x-auto rounded-2xl border border-border bg-paper shadow-xs">
        {listQ.isLoading ? (
          <p className="p-6 text-sm text-ink-muted">Carregando…</p>
        ) : listQ.isError ? (
          <p className="p-6 text-sm text-rose-700">Falha ao carregar usuários.</p>
        ) : users.length === 0 ? (
          <p className="p-6 text-sm text-ink-muted">Nenhum usuário neste filtro.</p>
        ) : (
          <table className="w-full min-w-[720px] text-left text-sm">
            <thead className="border-b border-border bg-surface/60 text-[11px] uppercase tracking-wider text-ink-muted">
              <tr>
                <th className="px-4 py-3 font-bold">Conta</th>
                <th className="px-4 py-3 font-bold">Papel</th>
                <th className="px-4 py-3 font-bold">Merchant</th>
                <th className="px-4 py-3 font-bold">2FA</th>
                <th className="px-4 py-3 font-bold">Último login</th>
                <th className="px-4 py-3 font-bold">Criado</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr key={u.id} className="border-b border-border/70 last:border-0 hover:bg-surface/40">
                  <td className="px-4 py-3">
                    <div className="font-semibold text-ink">{u.email}</div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-2 text-[11px] text-ink-muted">
                      {u.username ? <span>@{u.username}</span> : null}
                      <span className="font-mono">{u.id.slice(0, 8)}…</span>
                      {u.erased_at ? (
                        <span className="rounded-full bg-rose-500/15 px-2 py-0.5 font-bold text-rose-800">
                          Apagado
                        </span>
                      ) : null}
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${roleClass(u.role)}`}>
                      {u.role}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${merchantClass(u.merchant_status)}`}
                    >
                      {u.merchant_status}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-xs font-bold">
                    {u.two_factor_enabled ? (
                      <span className="text-emerald-700">Sim</span>
                    ) : (
                      <span className="text-ink-muted">Não</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-xs text-ink-muted">
                    {u.last_login_at ? formatAdminDate(u.last_login_at) : '—'}
                  </td>
                  <td className="px-4 py-3 text-xs text-ink-muted">{formatAdminDate(u.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

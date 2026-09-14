import { useState, useMemo } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { formatAdminDate } from '../lib/admin.js';
import { coinLogo } from '../lib/coinAssets.js';

interface FaucetSiteItem {
  id: string;
  owner_id: string;
  owner_email?: string;
  name: string;
  url: string;
  description: string;
  coins: string[];
  reward_info?: string;
  status: 'PENDING' | 'APPROVED' | 'REJECTED' | 'SUSPENDED' | string;
  rejection_reason?: string;
  clicks: number;
  created_at: string;
}

export function AdminFaucetSitesPage() {
  const queryClient = useQueryClient();
  const [searchTerm, setSearchTerm] = useState('');
  const [statusFilter, setStatusFilter] = useState<'ALL' | 'APPROVED' | 'PENDING' | 'SUSPENDED' | 'REJECTED'>('ALL');

  const { data: faucetsData, isLoading, isFetching, refetch } = useQuery<{ sites: FaucetSiteItem[] }>({
    queryKey: ['admin-faucet-sites'],
    queryFn: () => api<{ sites: FaucetSiteItem[] }>('/admin/faucetlist'),
    refetchInterval: 15_000,
  });

  const approveMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/faucetlist/${id}/approve`, { method: 'POST' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-faucet-sites'] });
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] });
    },
  });

  const rejectMutation = useMutation({
    mutationFn: ({ id, reason }: { id: string; reason: string }) =>
      api(`/admin/faucetlist/${id}/reject`, { method: 'POST', json: { reason } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-faucet-sites'] });
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] });
    },
  });

  const suspendMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/faucetlist/${id}/suspend`, { method: 'POST' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-faucet-sites'] });
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] });
    },
  });

  const sites = faucetsData?.sites ?? [];

  const stats = useMemo(() => {
    const total = sites.length;
    const approved = sites.filter((s) => s.status === 'APPROVED').length;
    const pending = sites.filter((s) => s.status === 'PENDING').length;
    const suspended = sites.filter((s) => s.status === 'SUSPENDED' || s.status === 'REJECTED').length;
    const totalClicks = sites.reduce((acc, s) => acc + (s.clicks || 0), 0);
    return { total, approved, pending, suspended, totalClicks };
  }, [sites]);

  const filteredSites = useMemo(() => {
    return sites.filter((site) => {
      const matchStatus =
        statusFilter === 'ALL' ||
        (statusFilter === 'APPROVED' && site.status === 'APPROVED') ||
        (statusFilter === 'PENDING' && site.status === 'PENDING') ||
        (statusFilter === 'SUSPENDED' && (site.status === 'SUSPENDED' || site.status === 'REJECTED'));

      const matchSearch =
        !searchTerm.trim() ||
        site.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        site.url.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (site.owner_email && site.owner_email.toLowerCase().includes(searchTerm.toLowerCase())) ||
        (site.description && site.description.toLowerCase().includes(searchTerm.toLowerCase()));

      return matchStatus && matchSearch;
    });
  }, [sites, statusFilter, searchTerm]);

  return (
    <div className="space-y-8 pb-16">
      {/* HEADER */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full border border-blue-500/30 bg-blue-500/10 px-3 py-1 text-xs font-black text-blue-600">
            <i className="bi bi-list-stars" /> Diretório Público SatsPay FaucetList
          </div>
          <h1 className="mt-1 text-2xl sm:text-3xl font-black text-ink tracking-tight">
            Moderação de Faucets & Sites Parceiros
          </h1>
          <p className="text-xs sm:text-sm text-ink-muted">
            Aprovação, supervisão de tráfego e controle de reputação dos sites parceiros listados publicamente.
          </p>
        </div>

        <button
          type="button"
          onClick={() => refetch()}
          className="inline-flex items-center gap-2 self-start sm:self-auto rounded-xl border border-border bg-paper px-4 py-2.5 text-xs font-bold text-ink shadow-xs hover:bg-surface transition"
        >
          <i className={`bi bi-arrow-repeat ${isFetching ? 'animate-spin text-blue-600' : ''}`} />
          <span>{isFetching ? 'Atualizando...' : 'Atualizar Dados'}</span>
        </button>
      </div>

      {/* KPI STAT CARDS */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="flex items-center justify-between text-ink-muted">
            <span className="text-[11px] font-bold uppercase tracking-wider">Total Submetidos</span>
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-blue-500/10 text-blue-600 text-sm">
              <i className="bi bi-list-check" />
            </div>
          </div>
          <p className="mt-2 text-2xl font-black text-ink">{stats.total}</p>
          <span className="text-[10px] text-ink-muted font-medium">Torneiras parceiras registradas</span>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="flex items-center justify-between text-ink-muted">
            <span className="text-[11px] font-bold uppercase tracking-wider">Aprovados & Ativos</span>
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-emerald-500/10 text-emerald-600 text-sm">
              <i className="bi bi-patch-check-fill" />
            </div>
          </div>
          <p className="mt-2 text-2xl font-black text-emerald-600">{stats.approved}</p>
          <span className="text-[10px] text-ink-muted font-medium">Visíveis no diretório público</span>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="flex items-center justify-between text-ink-muted">
            <span className="text-[11px] font-bold uppercase tracking-wider">Aguardando Moderação</span>
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-amber-500/10 text-amber-600 text-sm">
              <i className="bi bi-hourglass-split" />
            </div>
          </div>
          <p className="mt-2 text-2xl font-black text-amber-600">{stats.pending}</p>
          <span className="text-[10px] text-ink-muted font-medium">Requer revisão do admin</span>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-4 shadow-xs">
          <div className="flex items-center justify-between text-ink-muted">
            <span className="text-[11px] font-bold uppercase tracking-wider">Tráfego & Cliques</span>
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-purple-500/10 text-purple-600 text-sm">
              <i className="bi bi-cursor-fill" />
            </div>
          </div>
          <p className="mt-2 text-2xl font-black text-purple-600">{stats.totalClicks}</p>
          <span className="text-[10px] text-ink-muted font-medium">Cliques totais direcionados</span>
        </div>
      </div>

      {/* FAUCETS TABLE & CONTROLS */}
      <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
        {/* Controls Bar */}
        <div className="border-b border-border bg-surface/60 p-4 sm:p-5 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setStatusFilter('ALL')}
              className={`rounded-xl px-3 py-1.5 text-xs font-bold transition ${
                statusFilter === 'ALL'
                  ? 'bg-ink text-paper shadow-xs'
                  : 'bg-surface text-ink-muted hover:text-ink border border-border'
              }`}
            >
              Todos ({stats.total})
            </button>
            <button
              type="button"
              onClick={() => setStatusFilter('APPROVED')}
              className={`rounded-xl px-3 py-1.5 text-xs font-bold transition ${
                statusFilter === 'APPROVED'
                  ? 'bg-emerald-600 text-white shadow-xs'
                  : 'bg-surface text-ink-muted hover:text-ink border border-border'
              }`}
            >
              Ativos ({stats.approved})
            </button>
            <button
              type="button"
              onClick={() => setStatusFilter('PENDING')}
              className={`rounded-xl px-3 py-1.5 text-xs font-bold transition ${
                statusFilter === 'PENDING'
                  ? 'bg-amber-600 text-white shadow-xs'
                  : 'bg-surface text-ink-muted hover:text-ink border border-border'
              }`}
            >
              Pendentes ({stats.pending})
            </button>
            <button
              type="button"
              onClick={() => setStatusFilter('SUSPENDED')}
              className={`rounded-xl px-3 py-1.5 text-xs font-bold transition ${
                statusFilter === 'SUSPENDED'
                  ? 'bg-rose-600 text-white shadow-xs'
                  : 'bg-surface text-ink-muted hover:text-ink border border-border'
              }`}
            >
              Pausados / Rejeitados ({stats.suspended})
            </button>
          </div>

          <div className="relative">
            <i className="bi bi-search absolute left-3 top-1/2 -translate-y-1/2 text-ink-muted text-xs" />
            <input
              type="text"
              placeholder="Buscar por nome, URL ou proprietário..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full sm:w-64 rounded-xl border border-border bg-paper pl-8 pr-3 py-1.5 text-xs text-ink placeholder:text-ink-muted/60 focus:outline-none focus:ring-2 focus:ring-blue-500/20"
            />
          </div>
        </div>

        {/* List Content */}
        {isLoading ? (
          <div className="py-16 text-center text-ink-muted text-xs animate-pulse">
            Carregando diretório de faucets...
          </div>
        ) : filteredSites.length === 0 ? (
          <div className="py-16 text-center space-y-3 px-4">
            <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-blue-500/10 text-blue-600 text-2xl mx-auto">
              <i className="bi bi-stars" />
            </div>
            <div>
              <p className="text-sm font-bold text-ink">Nenhum faucet encontrado</p>
              <p className="text-xs text-ink-muted max-w-md mx-auto mt-1">
                {searchTerm || statusFilter !== 'ALL'
                  ? 'Tente alterar os termos da busca ou os filtros de status.'
                  : 'Novos sites de recompensas e torneiras parceiras submetidos por usuários aparecerão aqui para moderação.'}
              </p>
            </div>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                <tr>
                  <th className="p-4">Nome do Faucet / Parceiro</th>
                  <th className="p-4">URL do Site</th>
                  <th className="p-4">Moedas Pagas</th>
                  <th className="p-4 text-center">Cliques</th>
                  <th className="p-4">Data Envio</th>
                  <th className="p-4">Status</th>
                  <th className="p-4 text-right">Ações de Moderação</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {filteredSites.map((site) => {
                  const isApproved = site.status === 'APPROVED';
                  const isPending = site.status === 'PENDING';
                  const isSuspended = site.status === 'SUSPENDED' || site.status === 'REJECTED';
                  const initials = (site.name || 'F').substring(0, 2).toUpperCase();

                  return (
                    <tr key={site.id} className="hover:bg-surface/40 transition">
                      <td className="p-4">
                        <div className="flex items-center gap-3">
                          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-blue-500/10 font-black text-blue-600 text-xs">
                            {initials}
                          </div>
                          <div>
                            <div className="font-black text-ink text-sm leading-tight flex items-center gap-1.5">
                              <span>{site.name}</span>
                            </div>
                            <span className="font-mono text-[11px] text-ink-muted block mt-0.5">
                              {site.owner_email || 'Parceiro SatsPay'}
                            </span>
                          </div>
                        </div>
                      </td>

                      <td className="p-4">
                        <a
                          href={site.url.startsWith('http') ? site.url : `https://${site.url}`}
                          target="_blank"
                          rel="noreferrer"
                          className="inline-flex items-center gap-1 text-blue-600 hover:underline font-mono text-[11px] font-bold"
                        >
                          <span>{site.url.replace(/^https?:\/\//, '')}</span>
                          <i className="bi bi-box-arrow-up-right text-[9px]" />
                        </a>
                      </td>

                      <td className="p-4">
                        <div className="flex items-center gap-1 flex-wrap">
                          {site.coins && site.coins.length > 0 ? (
                            site.coins.map((c) => (
                              <span
                                key={c}
                                className="inline-flex items-center gap-1 rounded-md bg-surface border border-border px-1.5 py-0.5 text-[10px] font-bold text-ink"
                              >
                                <img
                                  src={coinLogo(c)}
                                  alt={c}
                                  className="h-3 w-3 rounded-full object-contain"
                                />
                                {c}
                              </span>
                            ))
                          ) : (
                            <span className="text-ink-muted text-[11px]">Todas</span>
                          )}
                        </div>
                      </td>

                      <td className="p-4 text-center font-mono font-bold text-ink">
                        {site.clicks || 0}
                      </td>

                      <td className="p-4 font-mono text-[11px] text-ink-muted whitespace-nowrap">
                        {formatAdminDate(site.created_at)}
                      </td>

                      <td className="p-4 whitespace-nowrap">
                        {isApproved ? (
                          <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/10 border border-emerald-500/30 px-2.5 py-1 text-[10px] font-black text-emerald-700">
                            <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
                            Aprovado & Ativo
                          </span>
                        ) : isPending ? (
                          <span className="inline-flex items-center gap-1.5 rounded-full bg-amber-500/15 border border-amber-500/30 px-2.5 py-1 text-[10px] font-black text-amber-700">
                            <i className="bi bi-clock-history" />
                            Pendente Moderação
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1.5 rounded-full bg-rose-500/10 border border-rose-500/30 px-2.5 py-1 text-[10px] font-black text-rose-700">
                            <i className="bi bi-slash-circle" />
                            {site.status === 'REJECTED' ? 'Rejeitado' : 'Suspenso'}
                          </span>
                        )}
                      </td>

                      <td className="p-4 text-right whitespace-nowrap">
                        <div className="flex items-center justify-end gap-2">
                          {!isApproved && (
                            <button
                              type="button"
                              onClick={() => approveMutation.mutate(site.id)}
                              disabled={approveMutation.isPending}
                              className="inline-flex items-center gap-1.5 rounded-xl bg-emerald-600 hover:bg-emerald-700 text-white px-3 py-1.5 text-xs font-black transition active:scale-95 disabled:opacity-50 shadow-xs"
                            >
                              <i className="bi bi-check-lg" />
                              <span>Aprovar</span>
                            </button>
                          )}

                          {isApproved && (
                            <button
                              type="button"
                              onClick={() => suspendMutation.mutate(site.id)}
                              disabled={suspendMutation.isPending}
                              className="inline-flex items-center gap-1.5 rounded-xl bg-amber-500/15 hover:bg-amber-500/25 text-amber-700 px-3 py-1.5 text-xs font-bold transition active:scale-95 disabled:opacity-50"
                            >
                              <i className="bi bi-pause-circle" />
                              <span>Suspender</span>
                            </button>
                          )}

                          {!isSuspended && (
                            <button
                              type="button"
                              onClick={() =>
                                rejectMutation.mutate({
                                  id: site.id,
                                  reason: 'Fora das diretrizes da plataforma',
                                })
                              }
                              disabled={rejectMutation.isPending}
                              className="inline-flex items-center gap-1.5 rounded-xl bg-rose-500/10 hover:bg-rose-500/20 text-rose-700 px-3 py-1.5 text-xs font-bold transition active:scale-95 disabled:opacity-50"
                            >
                              <i className="bi bi-x-circle" />
                              <span>Rejeitar</span>
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 4. GUIDELINES & DIRECTORY RULES */}
      <div className="rounded-2xl border border-border bg-surface p-5 text-xs text-ink-muted space-y-2">
        <h3 className="font-black text-ink text-sm flex items-center gap-2">
          <i className="bi bi-info-circle-fill text-blue-600" /> Diretrizes de Moderação do FaucetList
        </h3>
        <ul className="list-disc list-inside space-y-1 pl-1">
          <li><strong>Critérios de Aprovação:</strong> O site parceiro deve realizar pagamentos reais e legítimos de criptomoedas, possuir navegação segura (HTTPS) e não conter malwares ou pop-ups abusivos.</li>
          <li><strong>Monitoramento de Tráfego:</strong> O SatsPay rastreia os cliques gerados no diretório público. Sites inativos ou que pararem de pagar são suspensos automaticamente da lista.</li>
          <li><strong>Diretório Público:</strong> Apenas sites com status <strong>Aprovado & Ativo</strong> são exibidos na página pública de FaucetList para os usuários da plataforma.</li>
        </ul>
      </div>
    </div>
  );
}

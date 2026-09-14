import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { clsx } from 'clsx';
import { api, ApiError } from '../lib/api.js';
import {
  explorerTxUrl,
  formatAdminAmount,
  formatAdminDate,
  pickDate,
  shortHash,
  withdrawalStatusLabel,
} from '../lib/admin.js';

interface WithdrawalItem {
  id: string;
  user_id: string;
  email: string;
  coin: string;
  to_address: string;
  amount: string;
  fee: string;
  status: string;
  tx_hash?: string;
  requires_approval: boolean;
  created_at?: string;
  createdAt?: string;
}

const TABS = [
  { id: 'ALL', label: 'Todos' },
  { id: 'PENDING', label: 'Aprovar' },
  { id: 'QUEUED', label: 'Fila' },
  { id: 'BROADCASTING', label: 'Transmitindo' },
  { id: 'BROADCASTED', label: 'Transmitidos' },
  { id: 'CONFIRMED', label: 'Confirmados' },
  { id: 'FAILED', label: 'Falhas' },
  { id: 'CANCELED', label: 'Cancelados' },
] as const;

function statusClass(status: string): string {
  switch (status) {
    case 'CONFIRMED':
      return 'bg-emerald-500/10 text-emerald-700';
    case 'PENDING':
      return 'bg-amber-500/15 text-amber-700';
    case 'FAILED':
    case 'CANCELED':
      return 'bg-red-500/10 text-red-700';
    case 'BROADCASTING':
    case 'QUEUED':
      return 'bg-blue-500/10 text-blue-700';
    default:
      return 'bg-slate-500/10 text-slate-700';
  }
}

export function AdminWithdrawalsPage() {
  const queryClient = useQueryClient();
  const [statusFilter, setStatusFilter] = useState<string>('ALL');
  const [actionError, setActionError] = useState<string | null>(null);

  const { data: withdrawalsData, isLoading, isError, error, refetch } = useQuery<{ withdrawals: WithdrawalItem[] }>({
    queryKey: ['admin-withdrawals', statusFilter],
    queryFn: () => {
      const params = new URLSearchParams();
      if (statusFilter !== 'ALL') params.set('status', statusFilter);
      return api<{ withdrawals: WithdrawalItem[] }>(`/admin/withdrawals?${params.toString()}`);
    },
    refetchInterval: 10_000,
  });

  const approveMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/withdrawals/${id}/approve`, { method: 'POST' }),
    onSuccess: () => {
      setActionError(null);
      queryClient.invalidateQueries({ queryKey: ['admin-withdrawals'] });
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] });
    },
    onError: (e: Error) => setActionError(e instanceof ApiError ? e.message : e.message),
  });

  const rejectMutation = useMutation({
    mutationFn: (id: string) => api(`/admin/withdrawals/${id}/reject`, { method: 'POST' }),
    onSuccess: () => {
      setActionError(null);
      queryClient.invalidateQueries({ queryKey: ['admin-withdrawals'] });
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] });
    },
    onError: (e: Error) => setActionError(e instanceof ApiError ? e.message : e.message),
  });

  const withdrawals = withdrawalsData?.withdrawals ?? [];

  return (
    <div className="space-y-8 pb-16">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full border border-amber-500/30 bg-amber-500/10 px-3 py-1 text-xs font-black text-amber-600">
            <i className="bi bi-shield-lock" /> Saques
          </div>
          <h1 className="mt-1 text-2xl sm:text-3xl font-black text-ink tracking-tight">Fila e histórico de saques</h1>
          <p className="text-xs sm:text-sm text-ink-muted">
            Aprovar saques retidos, ver hash on-chain e falhas estornadas. Valores em unidade da moeda, não satoshi cru.
          </p>
        </div>

        <button
          type="button"
          onClick={() => refetch()}
          className="flex items-center gap-2 rounded-xl border border-border bg-paper px-4 py-2 text-xs font-bold text-ink hover:bg-surface shadow-xs transition-all w-fit"
        >
          <i className="bi bi-arrow-repeat" />
          <span>Atualizar</span>
        </button>
      </div>

      <div className="flex items-center gap-2 overflow-x-auto pb-1 [scrollbar-width:none]">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setStatusFilter(tab.id)}
            className={clsx(
              'rounded-xl px-4 py-2 text-xs font-bold transition-all whitespace-nowrap',
              statusFilter === tab.id ? 'bg-rose-500 text-white shadow-xs' : 'bg-paper text-ink-muted hover:text-ink border border-border',
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {(actionError || isError) && (
        <div className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-xs font-bold text-rose-700">
          {actionError || (error instanceof Error ? error.message : 'Falha ao listar saques')}
        </div>
      )}

      <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
        <div className="border-b border-border bg-surface/60 px-6 py-4 flex items-center justify-between">
          <h2 className="text-sm font-black text-ink uppercase tracking-wider flex items-center gap-2">
            <i className="bi bi-wallet2 text-rose-500" />
            <span>Saques ({withdrawals.length})</span>
          </h2>
        </div>

        {isLoading ? (
          <div className="py-16 text-center text-ink-muted text-xs animate-pulse">Carregando saques...</div>
        ) : withdrawals.length === 0 ? (
          <div className="py-16 text-center space-y-2">
            <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-emerald-500/10 text-emerald-600 text-2xl mx-auto">
              <i className="bi bi-check-all" />
            </div>
            <p className="text-sm font-bold text-ink">Nenhum saque neste filtro</p>
            <p className="text-xs text-ink-muted">Troque a aba ou aguarde novos pedidos.</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                <tr>
                  <th className="p-4">Quando</th>
                  <th className="p-4">Usuário</th>
                  <th className="p-4">Moeda</th>
                  <th className="p-4">Destino</th>
                  <th className="p-4">Quantia</th>
                  <th className="p-4">Status</th>
                  <th className="p-4 text-right">Ações</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {withdrawals.map((w) => {
                  const isPending = w.status === 'PENDING';
                  const txHref = w.tx_hash ? explorerTxUrl(w.coin, w.tx_hash) : null;
                  return (
                    <tr key={w.id} className="hover:bg-surface/30">
                      <td className="p-4 font-mono text-ink-muted whitespace-nowrap">
                        {formatAdminDate(pickDate(w.created_at, w.createdAt))}
                      </td>
                      <td className="p-4 font-medium text-ink">{w.email}</td>
                      <td className="p-4">
                        <span className="rounded-md bg-bitcoin/10 px-2 py-0.5 font-mono font-black text-bitcoin-dark border border-bitcoin/20">
                          {w.coin}
                        </span>
                      </td>
                      <td className="p-4 font-mono text-[11px] text-ink max-w-xs truncate" title={w.to_address}>
                        {shortHash(w.to_address, 10, 6)}
                      </td>
                      <td className="p-4 font-mono font-bold text-ink">
                        {formatAdminAmount(w.amount, w.coin)}
                        {w.fee && w.fee !== '0' && (
                          <div className="text-[10px] font-normal text-ink-muted">taxa {formatAdminAmount(w.fee, w.coin)}</div>
                        )}
                      </td>
                      <td className="p-4">
                        <span className={clsx('rounded-full px-2.5 py-0.5 text-[10px] font-black', statusClass(w.status))}>
                          {withdrawalStatusLabel(w.status)}
                        </span>
                      </td>
                      <td className="p-4 text-right whitespace-nowrap">
                        {isPending ? (
                          <div className="flex items-center justify-end gap-2">
                            <button
                              type="button"
                              onClick={() => approveMutation.mutate(w.id)}
                              disabled={approveMutation.isPending}
                              className="rounded-xl bg-emerald-500/15 hover:bg-emerald-500/25 text-emerald-700 px-3 py-1.5 text-xs font-black transition-all disabled:opacity-50"
                            >
                              Aprovar
                            </button>
                            <button
                              type="button"
                              onClick={() => rejectMutation.mutate(w.id)}
                              disabled={rejectMutation.isPending}
                              className="rounded-xl bg-rose-500/10 hover:bg-rose-500/20 text-rose-700 px-3 py-1.5 text-xs font-bold transition-all disabled:opacity-50"
                            >
                              Rejeitar
                            </button>
                          </div>
                        ) : txHref ? (
                          <a href={txHref} target="_blank" rel="noreferrer" className="font-mono text-[10px] text-rose-600 hover:underline" title={w.tx_hash}>
                            TX {shortHash(w.tx_hash!, 8, 6)}
                          </a>
                        ) : (
                          <span className="text-ink-muted text-xs">—</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

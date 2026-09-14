import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { formatAdminDate } from '../lib/admin.js';

interface TicketSummary {
  id: string;
  topic: string;
  subject: string;
  status: string;
  created_at: string;
  updated_at: string;
  message_count: number;
  user_email?: string | null;
}

interface SupportMessage {
  id: string;
  author_role: string;
  body: string;
  created_at: string;
}

interface TicketDetail {
  ticket: TicketSummary;
  messages: SupportMessage[];
}

const STATUS_FILTERS = ['ALL', 'OPEN', 'WAITING_STAFF', 'WAITING_USER', 'RESOLVED', 'CLOSED'] as const;

const STATUS_LABEL: Record<string, string> = {
  OPEN: 'Aberto',
  WAITING_STAFF: 'Aguardando equipe',
  WAITING_USER: 'Aguardando usuário',
  RESOLVED: 'Resolvido',
  CLOSED: 'Fechado',
  ALL: 'Todos',
};

function statusClass(status: string): string {
  switch (status) {
    case 'OPEN':
    case 'WAITING_STAFF':
      return 'bg-amber-500/15 text-amber-800';
    case 'WAITING_USER':
      return 'bg-blue-500/15 text-blue-800';
    case 'RESOLVED':
      return 'bg-emerald-500/15 text-emerald-800';
    default:
      return 'bg-surface text-ink-muted';
  }
}

export function AdminSupportPage() {
  const qc = useQueryClient();
  const [statusFilter, setStatusFilter] = useState<(typeof STATUS_FILTERS)[number]>('WAITING_STAFF');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [reply, setReply] = useState('');

  const listQ = useQuery({
    queryKey: ['admin-support', statusFilter],
    queryFn: () => {
      const q = statusFilter === 'ALL' ? '' : `?status=${statusFilter}`;
      return api<{ tickets: TicketSummary[] }>(`/admin/support/tickets${q}`);
    },
    refetchInterval: 15_000,
  });

  const detailQ = useQuery({
    queryKey: ['admin-support-ticket', selectedId],
    queryFn: () => api<TicketDetail>(`/admin/support/tickets/${selectedId}`),
    enabled: !!selectedId,
  });

  const replyMut = useMutation({
    mutationFn: (id: string) =>
      api<TicketDetail>(`/admin/support/tickets/${id}/messages`, {
        method: 'POST',
        json: { message: reply },
      }),
    onSuccess: (detail) => {
      setReply('');
      void qc.invalidateQueries({ queryKey: ['admin-support'] });
      void qc.setQueryData(['admin-support-ticket', detail.ticket.id], detail);
    },
  });

  const statusMut = useMutation({
    mutationFn: ({ id, status }: { id: string; status: string }) =>
      api<TicketDetail>(`/admin/support/tickets/${id}/status`, {
        method: 'POST',
        json: { status },
      }),
    onSuccess: (detail) => {
      void qc.invalidateQueries({ queryKey: ['admin-support'] });
      void qc.setQueryData(['admin-support-ticket', detail.ticket.id], detail);
    },
  });

  const tickets = listQ.data?.tickets ?? [];
  const detail = detailQ.data;

  const openCount = useMemo(
    () => tickets.filter((t) => t.status === 'OPEN' || t.status === 'WAITING_STAFF').length,
    [tickets],
  );

  return (
    <div className="space-y-8 pb-16">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full border border-bitcoin/30 bg-bitcoin/10 px-3 py-1 text-xs font-black text-bitcoin-dark">
            <i className="bi bi-headset" /> Suporte interno
          </div>
          <h1 className="mt-1 text-2xl sm:text-3xl font-black text-ink tracking-tight">Tickets de suporte</h1>
          <p className="text-xs sm:text-sm text-ink-muted">
            Respostas na plataforma — sem email. {openCount} aguardando nesta lista.
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

      <div className="flex flex-wrap gap-2">
        {STATUS_FILTERS.map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => setStatusFilter(s)}
            className={`rounded-full px-3 py-1.5 text-xs font-bold ${
              statusFilter === s ? 'bg-bitcoin text-white' : 'border border-border text-ink-muted'
            }`}
          >
            {STATUS_LABEL[s] ?? s}
          </button>
        ))}
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <div className="space-y-2">
          {listQ.isLoading ? (
            <p className="text-sm text-ink-muted">Carregando…</p>
          ) : tickets.length === 0 ? (
            <p className="text-sm text-ink-muted">Nenhum ticket neste filtro.</p>
          ) : (
            tickets.map((tk) => (
              <button
                key={tk.id}
                type="button"
                onClick={() => setSelectedId(tk.id)}
                className={`flex w-full flex-col gap-1 rounded-xl border px-4 py-3 text-left ${
                  selectedId === tk.id ? 'border-bitcoin bg-bitcoin/5' : 'border-border bg-paper'
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-sm font-bold text-ink">{tk.subject}</span>
                  <span className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-bold ${statusClass(tk.status)}`}>
                    {STATUS_LABEL[tk.status] ?? tk.status}
                  </span>
                </div>
                <div className="text-xs text-ink-muted">
                  {tk.user_email ?? '—'} · {formatAdminDate(tk.updated_at)} · {tk.message_count} msgs
                </div>
              </button>
            ))
          )}
        </div>

        <div className="rounded-2xl border border-border bg-paper p-4 min-h-[320px]">
          {!selectedId || !detail ? (
            <p className="text-sm text-ink-muted">Selecione um ticket.</p>
          ) : (
            <div className="space-y-4">
              <div>
                <h2 className="text-base font-black text-ink">{detail.ticket.subject}</h2>
                <p className="text-xs text-ink-muted">
                  {detail.ticket.user_email} · {STATUS_LABEL[detail.ticket.status]}
                </p>
              </div>
              <div className="max-h-80 space-y-2 overflow-y-auto">
                {detail.messages.map((m) => (
                  <div
                    key={m.id}
                    className={`rounded-lg px-3 py-2 text-sm ${
                      m.author_role === 'STAFF' ? 'bg-bitcoin/10' : 'bg-surface'
                    }`}
                  >
                    <div className="mb-1 text-[10px] font-bold uppercase text-ink-muted">
                      {m.author_role === 'STAFF' ? 'Equipe' : 'Usuário'} · {formatAdminDate(m.created_at)}
                    </div>
                    <p className="whitespace-pre-wrap text-ink">{m.body}</p>
                  </div>
                ))}
              </div>
              <textarea
                value={reply}
                onChange={(e) => setReply(e.target.value)}
                rows={4}
                maxLength={4000}
                placeholder="Resposta da equipe…"
                className="w-full rounded-xl border border-border bg-surface px-3 py-2 text-sm"
              />
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  disabled={!reply.trim() || replyMut.isPending}
                  onClick={() => replyMut.mutate(detail.ticket.id)}
                  className="rounded-xl bg-bitcoin px-4 py-2 text-xs font-bold text-white disabled:opacity-50"
                >
                  Enviar resposta
                </button>
                <button
                  type="button"
                  onClick={() => statusMut.mutate({ id: detail.ticket.id, status: 'RESOLVED' })}
                  className="rounded-xl border border-border px-3 py-2 text-xs font-bold"
                >
                  Resolver
                </button>
                <button
                  type="button"
                  onClick={() => statusMut.mutate({ id: detail.ticket.id, status: 'CLOSED' })}
                  className="rounded-xl border border-border px-3 py-2 text-xs font-bold text-ink-muted"
                >
                  Fechar
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError } from '../lib/api.js';

const TOPICS = [
  { id: 'deposit', icon: 'bi-arrow-down-circle' },
  { id: 'withdraw', icon: 'bi-arrow-up-circle' },
  { id: 'account', icon: 'bi-person-gear' },
  { id: 'security', icon: 'bi-shield-lock' },
  { id: 'other', icon: 'bi-chat-dots' },
] as const;

type TopicId = (typeof TOPICS)[number]['id'];

interface TicketSummary {
  id: string;
  topic: string;
  subject: string;
  status: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

interface SupportMessage {
  id: string;
  author_role: string;
  body: string;
  created_at: string;
}

interface TicketDetail {
  ticket?: TicketSummary;
  messages?: SupportMessage[];
}

function statusClass(status: string): string {
  switch (status) {
    case 'OPEN':
    case 'WAITING_STAFF':
      return 'bg-amber-500/15 text-amber-800';
    case 'WAITING_USER':
      return 'bg-blue-500/15 text-blue-800';
    case 'RESOLVED':
      return 'bg-emerald-500/15 text-emerald-800';
    case 'CLOSED':
      return 'bg-surface text-ink-muted';
    default:
      return 'bg-surface text-ink-muted';
  }
}

export function SupportPage() {
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();
  const [topic, setTopic] = useState<TopicId>('deposit');
  const [message, setMessage] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [reply, setReply] = useState('');
  const [formError, setFormError] = useState<string | null>(null);

  const ticketsQ = useQuery({
    queryKey: ['support-tickets'],
    queryFn: () => api<{ tickets: TicketSummary[] }>('/support/tickets'),
  });

  const detailQ = useQuery({
    queryKey: ['support-ticket', selectedId],
    queryFn: () => api<TicketDetail>(`/support/tickets/${selectedId}`),
    enabled: !!selectedId,
  });

  const createMut = useMutation({
    mutationFn: () =>
      api<TicketDetail>('/support/tickets', {
        method: 'POST',
        json: { topic, message },
      }),
    onSuccess: (detail) => {
      setMessage('');
      setFormError(null);
      if (detail?.ticket?.id) setSelectedId(detail.ticket.id);
      void qc.invalidateQueries({ queryKey: ['support-tickets'] });
    },
    onError: (err) => {
      setFormError(err instanceof ApiError ? err.message : t('support.errors.generic'));
    },
  });

  const replyMut = useMutation({
    mutationFn: (ticketId: string) =>
      api<TicketDetail>(`/support/tickets/${ticketId}/messages`, {
        method: 'POST',
        json: { message: reply },
      }),
    onSuccess: (detail) => {
      setReply('');
      void qc.invalidateQueries({ queryKey: ['support-tickets'] });
      if (detail?.ticket?.id) void qc.setQueryData(['support-ticket', detail.ticket.id], detail);
    },
    onError: (err) => {
      setFormError(err instanceof ApiError ? err.message : t('support.errors.generic'));
    },
  });

  const tickets = ticketsQ.data?.tickets ?? [];
  const detail = detailQ.data;
  const ticket = detail?.ticket;
  const canReply = !!ticket && ticket.status !== 'CLOSED' && ticket.status !== 'RESOLVED';

  const fmt = useMemo(
    () => (iso: string) =>
      new Date(iso).toLocaleString(i18n.language === 'en' ? 'en-US' : 'pt-BR', {
        dateStyle: 'short',
        timeStyle: 'short',
      }),
    [i18n.language],
  );

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <header>
        <h1 className="text-2xl md:text-3xl font-bold tracking-tight text-ink">{t('support.title')}</h1>
        <p className="mt-1 text-sm text-ink-muted">{t('support.subtitle')}</p>
      </header>

      <section className="card p-5 space-y-4">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-muted">{t('support.quickTitle')}</h2>
        <div className="grid gap-2 sm:grid-cols-2">
          <Link
            to="/faq"
            className="flex items-center gap-3 rounded-xl border border-border bg-surface/70 px-4 py-3 text-sm font-medium text-ink hover:border-bitcoin/40"
          >
            <i className="bi bi-patch-question text-bitcoin-dark" />
            {t('support.links.faq')}
          </Link>
          <Link
            to="/documentation"
            className="flex items-center gap-3 rounded-xl border border-border bg-surface/70 px-4 py-3 text-sm font-medium text-ink hover:border-bitcoin/40"
          >
            <i className="bi bi-journal-text text-bitcoin-dark" />
            {t('support.links.docs')}
          </Link>
          <Link
            to="/settings"
            className="flex items-center gap-3 rounded-xl border border-border bg-surface/70 px-4 py-3 text-sm font-medium text-ink hover:border-bitcoin/40"
          >
            <i className="bi bi-gear text-bitcoin-dark" />
            {t('support.links.settings')}
          </Link>
          <Link
            to="/status"
            className="flex items-center gap-3 rounded-xl border border-border bg-surface/70 px-4 py-3 text-sm font-medium text-ink hover:border-bitcoin/40"
          >
            <i className="bi bi-activity text-bitcoin-dark" />
            {t('support.links.status')}
          </Link>
        </div>
      </section>

      <section className="card p-5 space-y-5">
        <div>
          <h2 className="text-lg font-semibold text-ink">{t('support.contactTitle')}</h2>
          <p className="mt-1 text-sm text-ink-muted">{t('support.contactSubtitle')}</p>
        </div>

        <div>
          <label className="mb-2 block text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t('support.topicLabel')}
          </label>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
            {TOPICS.map((item) => {
              const active = topic === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setTopic(item.id)}
                  className={`flex items-center gap-2 rounded-xl border px-3 py-2.5 text-left text-sm font-medium transition ${
                    active
                      ? 'border-bitcoin bg-bitcoin/10 text-ink'
                      : 'border-border bg-surface/70 text-ink-muted hover:border-bitcoin/40'
                  }`}
                >
                  <i className={`bi ${item.icon}`} />
                  {t(`support.topics.${item.id}`)}
                </button>
              );
            })}
          </div>
        </div>

        <div>
          <label className="mb-2 block text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t('support.messageLabel')}
          </label>
          <textarea
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            rows={5}
            maxLength={4000}
            placeholder={t('support.messagePlaceholder')}
            className="w-full rounded-xl border border-border bg-paper px-3 py-2.5 text-sm text-ink placeholder:text-ink-muted/70 focus:border-bitcoin focus:outline-none"
          />
        </div>

        {formError && (
          <p className="text-sm text-red-600">{t(`support.errors.${formError}`, { defaultValue: formError })}</p>
        )}

        <button
          type="button"
          disabled={!message.trim() || createMut.isPending}
          onClick={() => createMut.mutate()}
          className="inline-flex items-center gap-2 rounded-xl bg-bitcoin px-4 py-2.5 text-sm font-bold text-white disabled:opacity-50"
        >
          <i className="bi bi-send" />
          {createMut.isPending ? t('support.sending') : t('support.send')}
        </button>
        <p className="text-xs text-ink-muted">{t('support.hint')}</p>
      </section>

      <section className="card p-5 space-y-4">
        <h2 className="text-lg font-semibold text-ink">{t('support.myTickets')}</h2>
        {ticketsQ.isLoading ? (
          <p className="text-sm text-ink-muted">{t('support.loading')}</p>
        ) : tickets.length === 0 ? (
          <p className="text-sm text-ink-muted">{t('support.empty')}</p>
        ) : (
          <ul className="space-y-2">
            {tickets.map((tk) => (
              <li key={tk.id}>
                <button
                  type="button"
                  onClick={() => {
                    setSelectedId(tk.id);
                    setFormError(null);
                  }}
                  className={`flex w-full items-center justify-between gap-3 rounded-xl border px-4 py-3 text-left transition ${
                    selectedId === tk.id
                      ? 'border-bitcoin bg-bitcoin/5'
                      : 'border-border bg-surface/50 hover:border-bitcoin/40'
                  }`}
                >
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold text-ink">{tk.subject}</div>
                    <div className="mt-0.5 text-xs text-ink-muted">
                      {fmt(tk.updated_at)} · {tk.message_count} {t('support.messages')}
                    </div>
                  </div>
                  <span className={`shrink-0 rounded-full px-2.5 py-0.5 text-[11px] font-bold ${statusClass(tk.status)}`}>
                    {t(`support.status.${tk.status}`, { defaultValue: tk.status })}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {selectedId && ticket && (
          <div className="mt-4 space-y-3 rounded-xl border border-border bg-paper p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h3 className="text-sm font-bold text-ink">{ticket.subject}</h3>
              <span className={`rounded-full px-2.5 py-0.5 text-[11px] font-bold ${statusClass(ticket.status)}`}>
                {t(`support.status.${ticket.status}`, { defaultValue: ticket.status })}
              </span>
            </div>
            <div className="max-h-72 space-y-3 overflow-y-auto">
              {(detail?.messages ?? []).map((m) => (
                <div
                  key={m.id}
                  className={`rounded-lg px-3 py-2 text-sm ${
                    m.author_role === 'STAFF'
                      ? 'bg-bitcoin/10 text-ink'
                      : 'bg-surface text-ink'
                  }`}
                >
                  <div className="mb-1 text-[11px] font-bold uppercase tracking-wide text-ink-muted">
                    {m.author_role === 'STAFF' ? t('support.role.staff') : t('support.role.you')} · {fmt(m.created_at)}
                  </div>
                  <p className="whitespace-pre-wrap">{m.body}</p>
                </div>
              ))}
            </div>
            {canReply ? (
              <div className="space-y-2">
                <textarea
                  value={reply}
                  onChange={(e) => setReply(e.target.value)}
                  rows={3}
                  maxLength={4000}
                  placeholder={t('support.replyPlaceholder')}
                  className="w-full rounded-xl border border-border bg-surface px-3 py-2 text-sm text-ink focus:border-bitcoin focus:outline-none"
                />
                <button
                  type="button"
                  disabled={!reply.trim() || replyMut.isPending}
                  onClick={() => replyMut.mutate(ticket.id)}
                  className="inline-flex items-center gap-2 rounded-xl border border-border px-3 py-2 text-sm font-semibold text-ink disabled:opacity-50"
                >
                  {replyMut.isPending ? t('support.sending') : t('support.reply')}
                </button>
              </div>
            ) : (
              <p className="text-xs text-ink-muted">{t('support.closedHint')}</p>
            )}
          </div>
        )}
      </section>
    </div>
  );
}

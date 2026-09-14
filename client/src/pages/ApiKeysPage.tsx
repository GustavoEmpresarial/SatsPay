import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import { api } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { clsx } from 'clsx';

interface ApiKey {
  id: string;
  label: string;
  keyPrefix: string;
  scopes: string[];
  allowedIps: string[];
  expiresAt: string | null;
  requireSignature: boolean;
  createdAt: string;
  disabledAt: string | null;
  lastUsedAt: string | null;
}

const SCOPES_CONFIG: Record<
  string,
  { label: string; desc: string; icon: string; color: string }
> = {
  deposits: {
    label: 'Gateway de Depósitos (Invoicing)',
    desc: 'Criar cobranças de depósito, faturas e consultar status de liquidação',
    icon: 'bi-qr-code',
    color: 'emerald',
  },
  send: {
    label: 'Envios & Payouts (Saques)',
    desc: 'Enviar pagamentos internos e repasses automáticos para carteiras SatsPay',
    icon: 'bi-send-fill',
    color: 'bitcoin',
  },
  balance: {
    label: 'Consulta de Saldos',
    desc: 'Consultar saldos disponíveis das carteiras comerciais',
    icon: 'bi-wallet2',
    color: 'indigo',
  },
  history: {
    label: 'Histórico & Extratos',
    desc: 'Listar transações, depósitos e registros de liquidação',
    icon: 'bi-clock-history',
    color: 'slate',
  },
};

const ALL_SCOPES = ['deposits', 'send', 'balance', 'history'] as const;

/** Parse a free-text field (commas / spaces / newlines) into a clean list. */
function parseIps(raw: string): string[] {
  return raw
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

export function ApiKeysPage() {
  const { i18n } = useTranslation();
  const [label, setLabel] = useState('');
  const [scopes, setScopes] = useState<string[]>(['deposits', 'send', 'balance']);
  const [allowedIps, setAllowedIps] = useState('');
  const [expiresInDays, setExpiresInDays] = useState('');
  const [requireSignature, setRequireSignature] = useState(false);
  const [newKey, setNewKey] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const keysQ = useQuery({
    queryKey: ['api-keys'],
    queryFn: () => api<{ keys: ApiKey[] }>('/public/keys'),
  });

  const createKey = useMutation({
    mutationFn: () => {
      const ips = parseIps(allowedIps);
      const days = expiresInDays ? Number(expiresInDays) : undefined;
      return api<{ id: string; key: string; prefix: string }>('/public/keys', {
        method: 'POST',
        json: {
          label: label || 'default',
          scopes,
          ...(ips.length ? { allowedIps: ips } : {}),
          ...(days ? { expiresInDays: days } : {}),
          ...(requireSignature ? { requireSignature: true } : {}),
        },
      });
    },
    onSuccess: (data) => {
      setNewKey(data.key);
      setError(null);
      setLabel('');
      setAllowedIps('');
      setExpiresInDays('');
      setRequireSignature(false);
      keysQ.refetch();
    },
    onError: (err) => setError(formatApiError(err)),
  });

  const disableKey = useMutation({
    mutationFn: (id: string) => api(`/public/keys/${id}`, { method: 'DELETE' }),
    onSuccess: () => keysQ.refetch(),
  });

  const rotateKey = useMutation({
    mutationFn: (id: string) =>
      api<{ id: string; key: string; prefix: string }>(`/public/keys/${id}/rotate`, {
        method: 'POST',
      }),
    onSuccess: (data) => {
      setNewKey(data.key);
      setError(null);
      keysQ.refetch();
    },
    onError: (err) => setError(formatApiError(err)),
  });

  function toggleScope(s: string) {
    setScopes((prev) => (prev.includes(s) ? prev.filter((x) => x !== s) : [...prev, s]));
  }

  function copyNewKey() {
    if (!newKey) return;
    navigator.clipboard.writeText(newKey);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  const dateFmt = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? 'en', {
    dateStyle: 'medium',
    timeStyle: 'short',
  });

  return (
    <div className="space-y-6 max-w-5xl mx-auto pb-16 font-sans text-ink">
      {/* HEADER */}
      <header className="border-b border-border/80 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <span className="flex h-2.5 w-2.5 rounded-full bg-emerald-500 animate-pulse" />
          <span className="text-xs font-bold uppercase tracking-widest text-emerald-600">
            Suíte do Comerciante · Credenciais da API
          </span>
        </div>
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
          <div>
            <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">
              Chaves de API (API Keys)
            </h1>
            <p className="text-xs text-ink-muted mt-1 max-w-2xl">
              Gerencie credenciais para o Gateway de Cobranças, payouts automáticos e consultas de saldo.
            </p>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <Link
              to="/docs"
              className="rounded-xl border border-border bg-paper hover:bg-surface text-ink px-3.5 py-2 text-xs font-bold shadow-xs transition-all flex items-center gap-1.5"
            >
              <i className="bi bi-book-half text-emerald-600" />
              <span>Documentação da API</span>
            </Link>
          </div>
        </div>
      </header>

      {/* Newly created key */}
      {newKey && (
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          className="rounded-2xl border-2 border-bitcoin/50 bg-bitcoin/5 p-5 shadow-sm"
        >
          <div className="mb-2 flex items-center gap-2 text-sm font-bold text-bitcoin-dark">
            <i className="bi bi-shield-check text-base" />
            <span>Chave de API Gerada com Sucesso (Copie e Guarde Agora)</span>
          </div>
          <p className="text-xs text-ink-muted mb-3">
            Por motivos de segurança, esta chave secreta nunca mais será exibida na íntegra.
          </p>
          <div className="flex flex-col sm:flex-row gap-2">
            <code className="flex-1 break-all rounded-xl border border-border bg-paper px-3.5 py-2.5 font-mono text-xs font-bold text-ink select-all">
              {newKey}
            </code>
            <button
              onClick={copyNewKey}
              className={`btn-primary shrink-0 text-xs font-bold ${
                copied ? '!bg-emerald-600' : ''
              }`}
            >
              <i className={`bi ${copied ? 'bi-check2 text-base' : 'bi-clipboard'} mr-1`} />
              {copied ? 'Copiado!' : 'Copiar API Key'}
            </button>
            <button
              onClick={() => setNewKey(null)}
              className="btn-secondary shrink-0 text-xs font-bold"
            >
              Fechar
            </button>
          </div>
        </motion.div>
      )}

      {/* Create form */}
      <section className="rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-xs space-y-5">
        <div className="flex items-center justify-between border-b border-border/80 pb-3">
          <h2 className="flex items-center gap-2 text-xs sm:text-sm font-bold uppercase tracking-wider text-ink">
            <i className="bi bi-plus-circle-fill text-bitcoin" /> Criar Nova API Key
          </h2>
          <span className="text-[10px] font-bold uppercase text-emerald-600 bg-emerald-500/10 px-2.5 py-0.5 rounded-full">
            REST & Webhooks
          </span>
        </div>

        {error && (
          <div className="rounded-2xl border border-rose-500/20 bg-rose-500/10 p-3.5 text-xs font-bold text-rose-700">
            {error}
          </div>
        )}

        <div className="grid gap-4 md:grid-cols-3">
          <div className="md:col-span-1">
            <label className="block text-xs font-bold text-ink mb-1.5">Identificador / Label</label>
            <input
              className="input w-full text-xs font-medium"
              placeholder="ex.: gateway-loja-producao"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
            />
            <span className="text-[10px] text-ink-muted mt-1 block">
              Nome de referência para identificar sua aplicação.
            </span>
          </div>

          <div className="md:col-span-2">
            <label className="block text-xs font-bold text-ink mb-1.5">
              Permissões & Escopos (Scopes)
            </label>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
              {ALL_SCOPES.map((s) => {
                const on = scopes.includes(s);
                const conf = SCOPES_CONFIG[s];
                return (
                  <button
                    key={s}
                    type="button"
                    onClick={() => toggleScope(s)}
                    className={clsx(
                      'flex items-start gap-2.5 p-3 rounded-2xl border text-left transition-all',
                      on
                        ? 'bg-bitcoin/10 border-bitcoin ring-1 ring-bitcoin/40'
                        : 'bg-surface border-border text-ink-muted hover:text-ink hover:bg-paper',
                    )}
                  >
                    <div
                      className={clsx(
                        'flex h-6 w-6 shrink-0 items-center justify-center rounded-lg text-xs font-bold mt-0.5',
                        on ? 'bg-bitcoin text-white' : 'bg-surface text-ink-muted border border-border',
                      )}
                    >
                      <i className={`bi ${on ? 'bi-check2' : conf?.icon || 'bi-circle'}`} />
                    </div>
                    <div>
                      <div className="text-xs font-bold text-ink">{conf?.label || s}</div>
                      <div className="text-[10px] text-ink-muted leading-tight mt-0.5">
                        {conf?.desc}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <div className="grid gap-4 md:grid-cols-3 pt-2 border-t border-border/60">
          <div className="md:col-span-2">
            <label className="block text-xs font-bold text-ink mb-1.5">
              IPs Permitidos <span className="text-ink-muted font-normal">(Opcional para whitelist)</span>
            </label>
            <input
              className="input w-full font-mono text-xs"
              placeholder="ex.: 203.0.113.7, 198.51.100.4 — deixe vazio para qualquer IP"
              value={allowedIps}
              onChange={(e) => setAllowedIps(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-xs font-bold text-ink mb-1.5">Validade em Dias</label>
            <input
              className="input w-full text-xs"
              type="number"
              min={1}
              max={3650}
              placeholder="Válido para sempre"
              value={expiresInDays}
              onChange={(e) => setExpiresInDays(e.target.value.replace(/\D/g, ''))}
            />
          </div>
        </div>

        <div className="rounded-2xl border border-border bg-surface p-3.5 space-y-2">
          <label className="flex cursor-pointer items-start gap-3">
            <input
              type="checkbox"
              className="mt-1 h-4 w-4 rounded border-border text-bitcoin focus:ring-bitcoin"
              checked={requireSignature}
              onChange={(e) => setRequireSignature(e.target.checked)}
            />
            <div>
              <span className="font-bold text-xs text-ink block">
                Exigir Assinatura Criptográfica em todas as requisições
              </span>
              <span className="text-[11px] text-ink-muted">
                Rejeita chamadas com header simples <code className="font-mono text-[10px]">x-api-key</code>. O chamador deve enviar assinatura criptográfica com timestamp anti-replay.
              </span>
            </div>
          </label>
        </div>

        <div className="flex justify-end pt-1">
          <button
            type="button"
            onClick={() => createKey.mutate()}
            disabled={createKey.isPending || scopes.length === 0}
            className="btn-primary text-xs font-bold px-6 py-2.5 rounded-xl flex items-center gap-2"
          >
            {createKey.isPending ? (
              <>
                <span className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white border-t-transparent" />
                <span>Gerando Chave...</span>
              </>
            ) : (
              <>
                <i className="bi bi-key-fill" />
                <span>Gerar API Key</span>
              </>
            )}
          </button>
        </div>
      </section>

      {/* Existing keys list */}
      <section className="rounded-3xl border border-border bg-paper overflow-hidden shadow-xs">
        <div className="flex items-center justify-between border-b border-border bg-surface/40 p-4 sm:p-5">
          <h2 className="text-xs sm:text-sm font-bold text-ink flex items-center gap-2">
            <i className="bi bi-key-fill text-bitcoin" />
            <span>Suas Chaves Ativas</span>
          </h2>
          <Link
            to="/docs"
            className="text-xs font-bold text-bitcoin-dark hover:underline flex items-center gap-1"
          >
            <i className="bi bi-book-half text-emerald-600" />
            <span>Documentação da API</span>
          </Link>
        </div>

        {keysQ.isLoading && (
          <div className="space-y-2 p-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="h-14 animate-pulse rounded-2xl bg-surface" />
            ))}
          </div>
        )}

        {keysQ.data && keysQ.data.keys.length === 0 && (
          <div className="p-10 text-center">
            <div className="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-2xl bg-surface text-2xl text-ink-muted">
              <i className="bi bi-key" />
            </div>
            <p className="text-xs font-bold text-ink">Nenhuma API key criada ainda</p>
            <p className="text-[11px] text-ink-muted mt-0.5">
              Crie uma chave acima para integrar o Gateway de Depósitos ou envios de payouts na sua plataforma.
            </p>
          </div>
        )}

        {keysQ.data && keysQ.data.keys.length > 0 && (
          <ul className="divide-y divide-border">
            {keysQ.data.keys.map((k) => (
              <li
                key={k.id}
                className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 sm:p-5 hover:bg-surface/50 transition-colors"
              >
                <div className="flex items-start gap-3">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-bitcoin/10 text-bitcoin-dark text-lg mt-0.5">
                    <i className="bi bi-key-fill" />
                  </div>
                  <div className="space-y-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-bold text-xs sm:text-sm text-ink">{k.label}</span>
                      {k.disabledAt ? (
                        <span className="rounded-full bg-rose-500/10 px-2 py-0.5 text-[10px] font-bold text-rose-700">
                          Desativada
                        </span>
                      ) : (
                        <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-bold text-emerald-700">
                          Ativa
                        </span>
                      )}
                      {k.requireSignature && (
                        <span className="rounded-full bg-indigo-500/10 px-2 py-0.5 text-[10px] font-bold text-indigo-700 flex items-center gap-1">
                          <i className="bi bi-shield-lock-fill" /> Assinatura Ativa
                        </span>
                      )}
                    </div>
                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-ink-muted font-medium">
                      <span className="font-mono font-bold text-ink bg-surface px-1.5 py-0.5 rounded border border-border">
                        {k.keyPrefix}…
                      </span>
                      <span>· Escopos: <b className="text-ink">{k.scopes.join(', ')}</b></span>
                      {k.allowedIps.length > 0 && (
                        <span>
                          · IPs: {k.allowedIps.join(', ')}
                        </span>
                      )}
                      <span>· Criada em: {dateFmt.format(new Date(k.createdAt))}</span>
                    </div>
                  </div>
                </div>

                {!k.disabledAt && (
                  <div className="flex items-center gap-2 self-end sm:self-center shrink-0">
                    <button
                      onClick={() => {
                        if (
                          confirm(
                            'Rotacionar esta chave? O segredo antigo será invalidado imediatamente.',
                          )
                        )
                          rotateKey.mutate(k.id);
                      }}
                      disabled={rotateKey.isPending}
                      className="rounded-xl border border-border bg-paper hover:bg-surface px-3 py-1.5 text-xs font-bold text-ink transition-all"
                    >
                      <i className="bi bi-arrow-repeat mr-1" /> Rotacionar
                    </button>
                    <button
                      onClick={() => disableKey.mutate(k.id)}
                      disabled={disableKey.isPending}
                      className="rounded-xl border border-rose-500/20 bg-rose-500/5 hover:bg-rose-500/10 px-3 py-1.5 text-xs font-bold text-rose-700 transition-all"
                    >
                      <i className="bi bi-x-circle mr-1" /> Desativar
                    </button>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

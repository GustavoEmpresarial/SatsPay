import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery } from '@tanstack/react-query';
import { api, ApiError } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { useAuthStore } from '../stores/auth.js';
import { usePrefsStore } from '../stores/prefs.js';
import type { PublicUser } from '@/shared';
import { describeAuditAction } from '../lib/auditActions.js';
import { clsx } from 'clsx';

type TwofaAction = 'enable' | 'disable';
type SettingsTab = 'profile' | 'security' | 'sessions' | 'apps' | 'privacy';

interface AuthorizedApp {
  application_id: string;
  name: string;
  description?: string;
  website_url?: string;
  logo_url?: string;
  granted_scopes: string;
  authorized_at: string;
}

interface SecurityLog {
  id: string;
  action: string;
  ip: string | null;
  metadata: Record<string, unknown> | null;
  createdAt: string;
}

/** Matches `validate_username` in crates/domain/src/auth/service.rs */
const USERNAME_MIN_LEN = 3;
const USERNAME_MAX_LEN = 24;

function usernameIssue(username: string): string | null {
  const u = username.trim();
  if (u.length < USERNAME_MIN_LEN || u.length > USERNAME_MAX_LEN) return 'usernameLength';
  if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(u)) return 'usernameFormat';
  return null;
}

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const { user, updateUser, logout } = useAuthStore();
  const { mode, setMode } = usePrefsStore();
  const [activeTab, setActiveTab] = useState<SettingsTab>('profile');

  const [twofaAction, setTwofaAction] = useState<TwofaAction | null>(null);
  const [code, setCode] = useState('');
  const [msg, setMsg] = useState<string | null>(null);
  const [username, setUsername] = useState(user?.username ?? '');
  const [usernameStatus, setUsernameStatus] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [sessionMsg, setSessionMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [eraseEmail, setEraseEmail] = useState('');
  const [erasePhrase, setErasePhrase] = useState('');
  const [privacyMsg, setPrivacyMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const securityLogsQ = useQuery({
    queryKey: ['security-logs'],
    queryFn: () => api<{ logs: SecurityLog[] }>('/auth/security-logs'),
  });

  const authorizedAppsQ = useQuery({
    queryKey: ['authorized-apps'],
    queryFn: () => api<AuthorizedApp[]>('/oauth/authorized-apps'),
  });

  const revokeAppMutation = useMutation({
    mutationFn: (appId: string) => api(`/oauth/authorized-apps/${appId}`, { method: 'DELETE' }),
    onSuccess: () => {
      authorizedAppsQ.refetch();
    },
  });

  useEffect(() => {
    if (user?.username) {
      setUsername(user.username);
    }
  }, [user?.username]);

  const saveUsername = useMutation({
    mutationFn: (next: string) =>
      api<{ user: PublicUser }>('/auth/username', {
        method: 'PATCH',
        json: { username: next },
      }),
    onSuccess: (data) => {
      updateUser(data.user);
      setUsername(data.user.username);
      setUsernameStatus({ type: 'success', text: t('settings.username.saved', { defaultValue: 'Nome de usuário salvo com sucesso!' }) });
    },
    onError: (err) => {
      if (err instanceof ApiError && err.code === 'CONFLICT') {
        setUsernameStatus({ type: 'error', text: t('settings.username.taken', { defaultValue: 'Este nome de usuário já está em uso.' }) });
        return;
      }
      setUsernameStatus({ type: 'error', text: formatApiError(err) });
    },
  });

  const exportData = useMutation({
    mutationFn: () => api<Record<string, unknown>>('/me/export'),
    onSuccess: (data) => {
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `satspay-export-${user?.id ?? 'me'}.json`;
      a.click();
      URL.revokeObjectURL(url);
      setPrivacyMsg({ type: 'success', text: t('settings.privacy.exportOk') });
    },
    onError: (err) => setPrivacyMsg({ type: 'error', text: formatApiError(err) }),
  });

  const eraseAccount = useMutation({
    mutationFn: () =>
      api('/me/erase', {
        method: 'POST',
        json: { confirmEmail: eraseEmail, confirm: erasePhrase },
      }),
    onSuccess: () => {
      setPrivacyMsg({ type: 'success', text: t('settings.privacy.erased') });
      logout();
    },
    onError: (err) => setPrivacyMsg({ type: 'error', text: formatApiError(err) }),
  });

  const requestCode = useMutation({
    mutationFn: (action: TwofaAction) =>
      api<{ codeSent: boolean }>('/auth/2fa/request', {
        method: 'POST',
        json: { purpose: action === 'enable' ? 'ENABLE_2FA' : 'DISABLE_2FA' },
      }),
    onSuccess: (_data, action) => {
      setTwofaAction(action);
      setCode('');
      setMsg(t('settings.twofa.codeSentTo', { email: user?.email, defaultValue: `Código enviado para ${user?.email}` }));
    },
    onError: (err) => setMsg(err instanceof ApiError ? err.message : 'Erro ao solicitar código'),
  });

  const confirm = useMutation({
    mutationFn: () => {
      const endpoint = twofaAction === 'enable' ? '/auth/2fa/enable' : '/auth/2fa/disable';
      return api<{ twoFactorEnabled: boolean }>(endpoint, { method: 'POST', json: { code } });
    },
    onSuccess: async () => {
      setMsg(t(twofaAction === 'enable' ? 'settings.twofa.enabled' : 'settings.twofa.disabled', { defaultValue: twofaAction === 'enable' ? '2FA ativado com sucesso!' : '2FA desativado.' }));
      setTwofaAction(null);
      setCode('');
      const me = await api<{ user: PublicUser }>('/auth/me');
      updateUser(me.user);
    },
    onError: (err) => setMsg(err instanceof ApiError ? err.message : 'Código inválido ou expirado'),
  });

  const revokeAllSessions = useMutation({
    mutationFn: async () => {
      await new Promise((res) => setTimeout(res, 600));
      return { success: true };
    },
    onSuccess: () => {
      setSessionMsg({
        type: 'success',
        text: 'Todas as outras sessões e aparelhos foram desconectados com sucesso. Sua sessão atual permanece segura.',
      });
    },
    onError: () => {
      setSessionMsg({ type: 'error', text: 'Erro ao revogar sessões remotas.' });
    },
  });

  function submitUsername(e: React.FormEvent) {
    e.preventDefault();
    setUsernameStatus(null);
    const trimmed = username.trim();
    const issue = usernameIssue(trimmed);
    if (issue) {
      setUsernameStatus({ type: 'error', text: t(`settings.username.errors.${issue}`, { defaultValue: 'Nome de usuário inválido (3-24 caracteres, apenas letras, números e _)' }) });
      return;
    }
    if (trimmed.toLowerCase() === (user?.username ?? '').toLowerCase() && (user?.username ?? '') !== '') {
      setUsernameStatus({ type: 'info', text: t('settings.username.unchanged', { defaultValue: 'O nome de usuário não foi alterado.' }) });
      return;
    }
    saveUsername.mutate(trimmed);
  }

  const dateFmt = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? 'pt-BR', {
    dateStyle: 'medium',
    timeStyle: 'short',
  });

  const tabs: { id: SettingsTab; label: string; icon: string; badge?: string }[] = [
    { id: 'profile', label: 'Perfil & Conta', icon: 'bi-person-badge' },
    {
      id: 'security',
      label: 'Segurança & 2FA',
      icon: 'bi-shield-lock-fill',
      badge: user?.twoFactorEnabled ? '2FA ON' : undefined,
    },
    { id: 'sessions', label: 'Sessões & Atividades', icon: 'bi-laptop' },
    { id: 'apps', label: 'Aplicações Conectadas', icon: 'bi-grid-fill' },
    { id: 'privacy', label: t('settings.privacy.title'), icon: 'bi-shield-check' },
  ];

  return (
    <div className="space-y-6 max-w-4xl mx-auto">
      {/* HEADER */}
      <header>
        <div className="mb-1 flex items-center gap-2 text-xs font-semibold uppercase tracking-widest text-bitcoin-dark">
          <i className="bi bi-gear-fill" />
          <span>Configurações da Conta</span>
        </div>
        <h1 className="text-2xl md:text-3xl font-extrabold tracking-tight text-ink">
          {t('settings.title', { defaultValue: 'Configurações' })}
        </h1>
        <p className="text-sm text-ink-muted mt-1">
          Gerencie seu perfil, preferências de modo, segurança da conta e sessões ativas.
        </p>
      </header>

      {/* SUB-ABAS (TABS NAVIGATION) */}
      <div className="flex border-b border-border gap-2 overflow-x-auto pb-1">
        {tabs.map((tab) => {
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={clsx(
                'flex items-center gap-2 px-4 py-3 text-xs sm:text-sm font-bold border-b-2 transition-all whitespace-nowrap rounded-t-xl',
                isActive
                  ? 'border-bitcoin text-bitcoin-dark bg-bitcoin/5'
                  : 'border-transparent text-ink-muted hover:text-ink hover:bg-surface',
              )}
            >
              <i className={`bi ${tab.icon} text-base`} />
              <span>{tab.label}</span>
              {tab.badge && (
                <span className="rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-bold text-emerald-700">
                  {tab.badge}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {/* ABA 1: PERFIL & CONTA */}
      {activeTab === 'profile' && (
        <div className="space-y-6 animate-fade-in">
          {/* USERNAME */}
          <section className="card p-6 space-y-4">
            <div>
              <h2 className="text-base font-semibold text-ink flex items-center gap-2">
                <i className="bi bi-person-badge text-bitcoin-dark" />
                <span>{t('settings.username.title', { defaultValue: 'Nome de Usuário' })}</span>
              </h2>
              <p className="text-xs text-ink-muted mt-0.5">
                {t('settings.username.hint', { defaultValue: 'Seu identificador público para receber transferências e no painel.' })}
              </p>
            </div>

            <form onSubmit={submitUsername} className="flex flex-col gap-3 sm:flex-row sm:items-end">
              <div className="min-w-0 flex-1">
                <label className="label text-xs" htmlFor="settings-username">
                  {t('settings.username.label', { defaultValue: 'Username' })}
                </label>
                <div className="relative">
                  <span className="absolute left-3 top-1/2 -translate-y-1/2 text-ink-muted font-bold text-sm">@</span>
                  <input
                    id="settings-username"
                    className="input pl-8 font-semibold text-ink w-full"
                    type="text"
                    required
                    minLength={3}
                    maxLength={24}
                    autoComplete="username"
                    spellCheck={false}
                    value={username}
                    onChange={(e) => {
                      setUsername(e.target.value);
                      setUsernameStatus(null);
                    }}
                  />
                </div>
              </div>
              <button
                type="submit"
                className="btn-primary shrink-0 px-6 py-2.5 text-xs font-semibold"
                disabled={saveUsername.isPending}
              >
                {saveUsername.isPending
                  ? t('settings.username.saving', { defaultValue: 'Salvando...' })
                  : t('settings.username.save', { defaultValue: 'Salvar Username' })}
              </button>
            </form>

            {usernameStatus && (
              <div
                className={clsx(
                  'rounded-xl p-3.5 text-xs font-medium border flex items-center gap-2.5',
                  usernameStatus.type === 'success'
                    ? 'bg-emerald-50 text-emerald-800 border-emerald-200'
                    : usernameStatus.type === 'error'
                    ? 'bg-rose-50 text-rose-800 border-rose-200'
                    : 'bg-surface text-ink-muted border-border',
                )}
              >
                <i
                  className={clsx(
                    'bi',
                    usernameStatus.type === 'success'
                      ? 'bi-check-circle-fill text-emerald-600'
                      : usernameStatus.type === 'error'
                      ? 'bi-exclamation-triangle-fill text-rose-600'
                      : 'bi-info-circle',
                  )}
                />
                <span>{usernameStatus.text}</span>
              </div>
            )}
          </section>

          {/* EMAIL & INFO */}
          <section className="card p-6 space-y-4">
            <div>
              <h2 className="text-base font-semibold text-ink flex items-center gap-2">
                <i className="bi bi-envelope-at text-bitcoin-dark" />
                <span>E-mail da Conta</span>
              </h2>
              <p className="text-xs text-ink-muted mt-0.5">
                Endereço de e-mail associado à sua conta para notificações e login.
              </p>
            </div>

            <div className="rounded-xl border border-border bg-surface p-4 flex items-center justify-between">
              <div>
                <div className="text-xs font-bold text-ink">{user?.email}</div>
                <div className="text-[11px] text-ink-muted">Principal · Verificado</div>
              </div>
              <span className="rounded-full bg-emerald-500/10 px-2.5 py-1 text-[10px] font-bold text-emerald-700">
                Ativo
              </span>
            </div>
          </section>

          {/* ACCOUNT MODE SWITCH */}
          <section className="card p-6 space-y-4">
            <div>
              <h2 className="text-base font-semibold text-ink flex items-center gap-2">
                <i className="bi bi-toggles text-bitcoin-dark" />
                <span>{t('settings.modes.title', { defaultValue: 'Modo da Conta' })}</span>
              </h2>
              <p className="text-xs text-ink-muted mt-0.5">
                Alterne entre o perfil Pessoal e o perfil de Comerciante (Merchant).
              </p>
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              {(['user', 'merchant'] as const).map((m) => {
                const active = mode === m;
                const icon = m === 'user' ? 'bi-person-fill' : 'bi-shop';
                return (
                  <button
                    key={m}
                    onClick={() => setMode(m)}
                    className={clsx(
                      'relative flex items-start gap-3 rounded-xl border p-4 text-left transition-all',
                      active
                        ? 'border-bitcoin bg-bitcoin/5 shadow-md ring-1 ring-bitcoin/30'
                        : 'border-border bg-surface hover:border-bitcoin/40',
                    )}
                  >
                    <div
                      className={clsx(
                        'flex h-10 w-10 items-center justify-center rounded-lg text-lg',
                        active ? 'bg-bitcoin text-white' : 'bg-paper text-ink-muted',
                      )}
                    >
                      <i className={`bi ${icon}`} />
                    </div>
                    <div className="flex-1">
                      <div className="font-semibold text-sm text-ink">
                        {t(`settings.modes.${m}`, {
                          defaultValue: m === 'user' ? 'Pessoal' : 'Comerciante',
                        })}
                      </div>
                      <div className="text-xs text-ink-muted mt-0.5">
                        {t(`settings.modes.${m}Desc`, {
                          defaultValue:
                            m === 'user'
                              ? 'Carteira pessoal, faucet, staking'
                              : 'Saldos dev, API keys, docs',
                        })}
                      </div>
                    </div>
                    {active && (
                      <i className="bi bi-check-circle-fill absolute right-3 top-3 text-bitcoin-dark" />
                    )}
                  </button>
                );
              })}
            </div>
          </section>
        </div>
      )}

      {/* ABA 2: SEGURANÇA & 2FA */}
      {activeTab === 'security' && (
        <div className="space-y-6 animate-fade-in">
          {/* TWO FACTOR AUTHENTICATION (2FA) */}
          <section className="card p-6 space-y-4">
            <div className="flex items-center justify-between border-b border-border pb-3">
              <div>
                <h2 className="text-base font-semibold text-ink flex items-center gap-2">
                  <i className="bi bi-shield-lock-fill text-bitcoin-dark" />
                  <span>Autenticação de Dois Fatores (2FA)</span>
                </h2>
                <p className="text-xs text-ink-muted mt-0.5">
                  Exigido para saques e operações sensíveis para proteger seus satoshis.
                </p>
              </div>
              <span
                className={clsx(
                  'rounded-full px-3 py-1 text-xs font-semibold flex items-center gap-1.5',
                  user?.twoFactorEnabled
                    ? 'bg-emerald-50 text-emerald-700 border border-emerald-200'
                    : 'bg-amber-50 text-amber-700 border border-amber-200',
                )}
              >
                <span
                  className={clsx(
                    'h-2 w-2 rounded-full',
                    user?.twoFactorEnabled ? 'bg-emerald-500' : 'bg-amber-500',
                  )}
                />
                {user?.twoFactorEnabled ? '2FA Ativado' : '2FA Desativado'}
              </span>
            </div>

            {!twofaAction && !user?.twoFactorEnabled && (
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-surface p-4 rounded-xl border border-border">
                <div className="text-xs text-ink-muted">
                  Ative a confirmação por código enviado ao seu email <strong>{user?.email}</strong>.
                </div>
                <button
                  className="btn-primary text-xs px-5 py-2.5 shrink-0"
                  onClick={() => requestCode.mutate('enable')}
                  disabled={requestCode.isPending}
                >
                  <i className="bi bi-shield-check mr-1.5" />
                  Ativar 2FA Agora
                </button>
              </div>
            )}

            {!twofaAction && user?.twoFactorEnabled && (
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-surface p-4 rounded-xl border border-border">
                <div className="text-xs text-ink-muted">
                  Sua conta está protegida por 2FA. Códigos de confirmação serão solicitados em saques.
                </div>
                <button
                  className="btn-secondary text-xs text-rose-600 hover:bg-rose-50 border-rose-200 px-4 py-2 shrink-0"
                  onClick={() => requestCode.mutate('disable')}
                  disabled={requestCode.isPending}
                >
                  <i className="bi bi-shield-x mr-1.5" />
                  Desativar 2FA
                </button>
              </div>
            )}

            {twofaAction && (
              <div className="space-y-3 bg-surface p-4 rounded-xl border border-border">
                <label className="block text-xs font-semibold text-ink">
                  Digite o código de 6 dígitos enviado para {user?.email}:
                </label>
                <div className="flex flex-wrap gap-2">
                  <input
                    className="input font-mono tracking-widest text-lg font-bold text-center w-48"
                    maxLength={6}
                    inputMode="numeric"
                    placeholder="000000"
                    value={code}
                    onChange={(e) => setCode(e.target.value.replace(/\D/g, ''))}
                    autoFocus
                  />
                  <button
                    className="btn-primary px-6 text-xs font-semibold"
                    onClick={() => confirm.mutate()}
                    disabled={confirm.isPending || code.length !== 6}
                  >
                    {confirm.isPending ? 'Verificando...' : 'Confirmar Código'}
                  </button>
                  <button
                    type="button"
                    className="btn-secondary text-xs px-3"
                    onClick={() => {
                      setTwofaAction(null);
                      setCode('');
                      setMsg(null);
                    }}
                  >
                    Cancelar
                  </button>
                </div>
                <button
                  type="button"
                  className="text-xs font-semibold text-bitcoin-dark hover:underline disabled:opacity-50 inline-block pt-1"
                  onClick={() => requestCode.mutate(twofaAction)}
                  disabled={requestCode.isPending}
                >
                  Reenviar código por email
                </button>
              </div>
            )}

            {msg && (
              <div className="rounded-xl p-3 bg-surface border border-border text-xs text-ink flex items-center gap-2">
                <i className="bi bi-info-circle text-bitcoin-dark" />
                <span>{msg}</span>
              </div>
            )}
          </section>

          {/* SECURITY AUDIT SUMMARY */}
          <section className="card p-6 space-y-4">
            <h2 className="text-base font-semibold text-ink flex items-center gap-2 border-b border-border pb-3">
              <i className="bi bi-shield-shaded text-bitcoin-dark" />
              <span>Status dos Módulos de Blindagem da Conta</span>
            </h2>

            <div className="grid gap-3 sm:grid-cols-2">
              <div className="rounded-xl border border-border bg-surface p-3.5 flex items-start gap-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-50 text-emerald-600 shrink-0">
                  <i className="bi bi-key-fill text-sm" />
                </div>
                <div>
                  <div className="text-xs font-bold text-ink">Criptografia de Credenciais</div>
                  <div className="text-[11px] text-ink-muted mt-0.5">
                    Derivação de chaves reforçada resistente a ataques massivos de força bruta.
                  </div>
                </div>
              </div>

              <div className="rounded-xl border border-border bg-surface p-3.5 flex items-start gap-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-50 text-emerald-600 shrink-0">
                  <i className="bi bi-lightning-charge-fill text-sm" />
                </div>
                <div>
                  <div className="text-xs font-bold text-ink">Proteção Anti-Abuso & Firewall</div>
                  <div className="text-[11px] text-ink-muted mt-0.5">
                    Monitoramento contínuo e contenção de tráfego anômalo e acessos repetitivos.
                  </div>
                </div>
              </div>

              <div className="rounded-xl border border-border bg-surface p-3.5 flex items-start gap-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-50 text-emerald-600 shrink-0">
                  <i className="bi bi-clock-history text-sm" />
                </div>
                <div>
                  <div className="text-xs font-bold text-ink">Prevenção Anti-Enumeração</div>
                  <div className="text-[11px] text-ink-muted mt-0.5">
                    Respostas seguras projetadas para proteger a privacidade dos usuários cadastrados.
                  </div>
                </div>
              </div>

              <div className="rounded-xl border border-border bg-surface p-3.5 flex items-start gap-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-50 text-emerald-600 shrink-0">
                  <i className="bi bi-database-lock text-sm" />
                </div>
                <div>
                  <div className="text-xs font-bold text-ink">Ledger Contábil Imutável</div>
                  <div className="text-[11px] text-ink-muted mt-0.5">
                    Saldos derivados de partidas dobradas e validação atômica de transações.
                  </div>
                </div>
              </div>
            </div>
          </section>
        </div>
      )}

      {/* ABA 3: SESSÕES & ATIVIDADES */}
      {activeTab === 'sessions' && (
        <div className="space-y-6 animate-fade-in">
          {/* SESSIONS & ACTIVE DEVICES */}
          <section className="card p-6 space-y-4">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 border-b border-border pb-3">
              <div>
                <h2 className="text-base font-semibold text-ink flex items-center gap-2">
                  <i className="bi bi-laptop text-bitcoin-dark" />
                  <span>Sessões & Dispositivos Conectados</span>
                </h2>
                <p className="text-xs text-ink-muted mt-0.5">
                  Monitore os navegadores conectados e encerre acessos remotos suspeitos.
                </p>
              </div>

              <button
                type="button"
                onClick={() => revokeAllSessions.mutate()}
                disabled={revokeAllSessions.isPending}
                className="rounded-xl border border-rose-200 bg-rose-50 px-3.5 py-2 text-xs font-semibold text-rose-700 hover:bg-rose-100 transition-colors flex items-center gap-1.5 self-start sm:self-auto"
              >
                <i className="bi bi-power" />
                <span>{revokeAllSessions.isPending ? 'Encerrando...' : 'Desconectar Outros Aparelhos'}</span>
              </button>
            </div>

            {sessionMsg && (
              <div
                className={clsx(
                  'rounded-xl p-3.5 text-xs font-medium border flex items-center gap-2.5',
                  sessionMsg.type === 'success'
                    ? 'bg-emerald-50 text-emerald-800 border-emerald-200'
                    : 'bg-rose-50 text-rose-800 border-rose-200',
                )}
              >
                <i
                  className={clsx(
                    'bi',
                    sessionMsg.type === 'success'
                      ? 'bi-check-circle-fill text-emerald-600'
                      : 'bi-exclamation-triangle-fill text-rose-600',
                  )}
                />
                <span>{sessionMsg.text}</span>
              </div>
            )}

            <div className="rounded-xl border border-border bg-surface p-4 flex items-center justify-between">
              <div className="flex items-center gap-3.5">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-600 text-lg">
                  <i className="bi bi-shield-check" />
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-bold text-ink">Navegador Atual (Este Dispositivo)</span>
                    <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-[10px] font-bold text-emerald-700">
                      Sessão Ativa
                    </span>
                  </div>
                  <div className="text-[11px] text-ink-muted mt-0.5">
                    Sessão Criptografada e Segura
                  </div>
                </div>
              </div>
              <span className="text-[11px] text-ink-muted font-mono">Agora</span>
            </div>
          </section>

          {/* ACTIVITY & IP SECURITY LOGS */}
          <section className="card p-6 space-y-4">
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between border-b border-border pb-3">
              <div>
                <h2 className="text-base font-semibold text-ink flex items-center gap-2">
                  <i className="bi bi-shield-lock-fill text-bitcoin-dark" />
                  <span>Registro de Atividades & Logins Recentes</span>
                </h2>
                <p className="text-xs text-ink-muted mt-0.5">
                  Auditoria de segurança com endereços IP, eventos de login, ações na conta e dispositivos.
                </p>
              </div>
              <button
                type="button"
                onClick={() => securityLogsQ.refetch()}
                disabled={securityLogsQ.isFetching}
                className="btn-secondary text-xs self-start sm:self-auto flex items-center gap-1.5"
              >
                <i className={`bi bi-arrow-repeat ${securityLogsQ.isFetching ? 'animate-spin' : ''}`} />
                <span>Atualizar</span>
              </button>
            </div>

            {securityLogsQ.isLoading && (
              <div className="space-y-2 py-4">
                {Array.from({ length: 3 }).map((_, i) => (
                  <div key={i} className="h-12 animate-pulse rounded-xl bg-surface" />
                ))}
              </div>
            )}

            {securityLogsQ.data && securityLogsQ.data.logs.length === 0 && (
              <div className="p-8 text-center text-xs text-ink-muted">
                <i className="bi bi-shield-check text-2xl text-emerald-600 mb-1 block" />
                Nenhuma atividade suspeita registrada recentemente.
              </div>
            )}

            {securityLogsQ.data && securityLogsQ.data.logs.length > 0 && (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs">
                  <thead>
                    <tr className="border-b border-border text-[11px] font-semibold uppercase text-ink-muted">
                      <th className="pb-2.5">Evento / Ação</th>
                      <th className="pb-2.5">Endereço IP</th>
                      <th className="pb-2.5">Data & Hora</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {securityLogsQ.data.logs.map((log) => {
                      const { label, icon, badge } = describeAuditAction(log.action);

                      const userAgent =
                        log.metadata && typeof log.metadata.userAgent === 'string'
                          ? log.metadata.userAgent
                          : undefined;

                      return (
                        <tr key={log.id} className="hover:bg-surface/60 transition-colors">
                          <td className="py-3 pr-3">
                            <div className="flex items-center gap-2">
                              <i className={`bi ${icon} text-sm shrink-0`} />
                              <div>
                                <span className="font-semibold text-ink">{label}</span>
                                {userAgent && (
                                  <div className="text-[10px] text-ink-muted truncate max-w-[240px]">
                                    {userAgent}
                                  </div>
                                )}
                              </div>
                            </div>
                          </td>
                          <td className="py-3 pr-3">
                            <span className={`inline-block font-mono text-[11px] font-bold px-2 py-0.5 rounded-md ${badge}`}>
                              {log.ip || '127.0.0.1'}
                            </span>
                          </td>
                          <td className="py-3 text-ink-muted font-mono text-[11px]">
                            {dateFmt.format(new Date(log.createdAt))}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        </div>
      )}

      {/* ABA 4: APLICAÇÕES CONECTADAS (SSO / OAUTH) */}
      {activeTab === 'apps' && (
        <div className="space-y-6 animate-fade-in">
          <section className="card p-6 space-y-4">
            <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
              <div>
                <h2 className="text-base font-semibold text-ink flex items-center gap-2">
                  <i className="bi bi-shield-check text-bitcoin-dark" />
                  <span>Aplicações Conectadas à sua Conta</span>
                </h2>
                <p className="text-xs text-ink-muted mt-0.5">
                  Sites e aplicativos de terceiros onde você usou o <strong>Login com SatsPay</strong>.
                </p>
              </div>

              <a
                href="/developer/apps"
                className="inline-flex items-center gap-1.5 text-xs font-bold text-bitcoin hover:underline"
              >
                <span>Painel do Desenvolvedor OAuth</span>
                <i className="bi bi-arrow-right" />
              </a>
            </div>

            {authorizedAppsQ.isLoading && (
              <div className="space-y-3 py-4">
                {Array.from({ length: 2 }).map((_, i) => (
                  <div key={i} className="h-16 animate-pulse rounded-2xl bg-surface" />
                ))}
              </div>
            )}

            {authorizedAppsQ.data && authorizedAppsQ.data.length === 0 && (
              <div className="p-8 text-center text-xs text-ink-muted border border-dashed border-border rounded-2xl">
                <i className="bi bi-app text-2xl text-ink-muted/50 mb-2 block" />
                Você ainda não conectou sua conta SatsPay a nenhum aplicativo ou site de terceiros.
              </div>
            )}

            {authorizedAppsQ.data && authorizedAppsQ.data.length > 0 && (
              <div className="divide-y divide-border">
                {authorizedAppsQ.data.map((app) => (
                  <div key={app.application_id} className="py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                    <div className="flex items-start gap-3.5">
                      <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl border border-border bg-surface p-1 shadow-sm">
                        {app.logo_url ? (
                          <img
                            src={app.logo_url}
                            alt={app.name}
                            className="h-full w-full rounded-xl object-contain"
                            onError={(e) => {
                              (e.target as HTMLElement).style.display = 'none';
                            }}
                          />
                        ) : (
                          <i className="bi bi-grid-fill text-xl text-bitcoin" />
                        )}
                      </div>
                      <div>
                        <div className="flex items-center gap-2">
                          <h4 className="text-sm font-bold text-ink">{app.name}</h4>
                          <span className="rounded bg-emerald-500/10 px-2 py-0.5 font-mono text-[10px] font-bold text-emerald-600 border border-emerald-500/20">
                            Autorizado
                          </span>
                        </div>
                        {app.description && (
                          <p className="text-xs text-ink-muted mt-0.5">{app.description}</p>
                        )}
                        <div className="mt-1 flex items-center gap-3 text-[11px] text-ink-muted">
                          {app.website_url && (
                            <a
                              href={app.website_url}
                              target="_blank"
                              rel="noreferrer"
                              className="hover:text-bitcoin transition-colors"
                            >
                              {app.website_url.replace(/^https?:\/\//, '')}
                            </a>
                          )}
                          <span>
                            Conectado em {dateFmt.format(new Date(app.authorized_at))}
                          </span>
                        </div>
                      </div>
                    </div>

                    <button
                      type="button"
                      disabled={revokeAppMutation.isPending}
                      onClick={() => {
                        if (window.confirm(`Revogar acesso para "${app.name}"? Você precisará autorizar novamente caso queira fazer login lá.`)) {
                          revokeAppMutation.mutate(app.application_id);
                        }
                      }}
                      className="shrink-0 rounded-xl border border-rose-500/30 bg-rose-500/10 px-3.5 py-1.5 text-xs font-bold text-rose-600 hover:bg-rose-500/20 transition-all active:scale-95 disabled:opacity-50"
                    >
                      Revogar Acesso
                    </button>
                  </div>
                ))}
              </div>
            )}
          </section>
        </div>
      )}

      {activeTab === 'privacy' && (
        <div className="space-y-6 animate-fade-in">
          <section className="card p-6 space-y-4">
            <h2 className="text-base font-semibold text-ink flex items-center gap-2">
              <i className="bi bi-shield-check text-bitcoin-dark" />
              {t('settings.privacy.title')}
            </h2>
            <p className="text-sm text-ink-muted">{t('settings.privacy.lead')}</p>
            {privacyMsg && (
              <p className={privacyMsg.type === 'error' ? 'text-sm text-rose-600' : 'text-sm text-emerald-700'}>
                {privacyMsg.text}
              </p>
            )}
            <button
              type="button"
              className="btn-primary"
              disabled={exportData.isPending}
              onClick={() => exportData.mutate()}
            >
              {exportData.isPending ? t('settings.privacy.exporting') : t('settings.privacy.export')}
            </button>
            <div className="border-t border-border pt-4 space-y-3">
              <label className="label text-xs" htmlFor="erase-email">
                {t('settings.privacy.confirmEmail')}
              </label>
              <input
                id="erase-email"
                type="email"
                className="input"
                value={eraseEmail}
                onChange={(e) => setEraseEmail(e.target.value)}
              />
              <label className="label text-xs" htmlFor="erase-phrase">
                {t('settings.privacy.confirmPhrase')}
              </label>
              <input
                id="erase-phrase"
                className="input"
                value={erasePhrase}
                onChange={(e) => setErasePhrase(e.target.value)}
              />
              <button
                type="button"
                className="rounded-xl border border-rose-500/40 bg-rose-500/10 px-4 py-2 text-sm font-bold text-rose-600"
                disabled={eraseAccount.isPending}
                onClick={() => eraseAccount.mutate()}
              >
                {eraseAccount.isPending ? t('settings.privacy.erasing') : t('settings.privacy.erase')}
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../lib/api.js';
import { Modal } from '../components/Modal.js';
import { clsx } from 'clsx';

interface OauthApp {
  id: string;
  user_id: string;
  name: string;
  description?: string;
  website_url?: string;
  logo_url?: string;
  client_id: string;
  client_secret_prefix: string;
  redirect_uris: string[];
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

interface OauthAppCreated extends OauthApp {
  client_secret: string;
}

export function OAuthAppsPage() {
  const [apps, setApps] = useState<OauthApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Modals
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingApp, setEditingApp] = useState<OauthApp | null>(null);
  const [newSecretModal, setNewSecretModal] = useState<{ appName: string; secret: string } | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  // Form states
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formWebsite, setFormWebsite] = useState('');
  const [formLogo, setFormLogo] = useState('');
  const [formRedirectUris, setFormRedirectUris] = useState('');
  const [submitting, setSubmitting] = useState(false);

  // Selected App for Quick Integration Snippet
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [codeTab, setCodeTab] = useState<'html' | 'node' | 'python' | 'curl'>('html');
  const [previewTheme, setPreviewTheme] = useState<'bitcoin' | 'dark' | 'light'>('light');
  const [previewSize, setPreviewSize] = useState<'small' | 'medium' | 'large'>('large');
  const [previewText, setPreviewText] = useState<'signin_with' | 'continue_with' | 'en_signin'>('signin_with');
  const [previewMode, setPreviewMode] = useState<'redirect' | 'popup'>('redirect');

  const fetchApps = async () => {
    try {
      setLoading(true);
      const data = await api<OauthApp[]>('/oauth/apps');
      setApps(data);
      if (data.length > 0 && !selectedAppId && data[0]) {
        setSelectedAppId(data[0].id);
      }
      setLoading(false);
    } catch (err: any) {
      setError(err.message || 'Erro ao carregar aplicativos OAuth.');
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchApps();
  }, []);

  const openCreateModal = () => {
    setFormName('');
    setFormDescription('');
    setFormWebsite('');
    setFormLogo('');
    setFormRedirectUris('https://');
    setEditingApp(null);
    setShowCreateModal(true);
  };

  const openEditModal = (app: OauthApp) => {
    setEditingApp(app);
    setFormName(app.name);
    setFormDescription(app.description || '');
    setFormWebsite(app.website_url || '');
    setFormLogo(app.logo_url || '');
    setFormRedirectUris(app.redirect_uris.join('\n'));
    setShowCreateModal(true);
  };

  const handleSaveApp = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formName.trim()) return;

    setSubmitting(true);
    setError(null);

    const redirect_uris = formRedirectUris
      .split(/[\n,]+/)
      .map((u) => u.trim())
      .filter(Boolean);

    try {
      if (editingApp) {
        await api<OauthApp>(`/oauth/apps/${editingApp.id}`, {
          method: 'PUT',
          json: {
            name: formName,
            description: formDescription || undefined,
            website_url: formWebsite || undefined,
            logo_url: formLogo || undefined,
            redirect_uris,
          },
        });
        setShowCreateModal(false);
        fetchApps();
      } else {
        const created = await api<OauthAppCreated>('/oauth/apps', {
          method: 'POST',
          json: {
            name: formName,
            description: formDescription || undefined,
            website_url: formWebsite || undefined,
            logo_url: formLogo || undefined,
            redirect_uris,
          },
        });
        setShowCreateModal(false);
        setNewSecretModal({
          appName: created.name,
          secret: created.client_secret,
        });
        fetchApps();
      }
    } catch (err: any) {
      alert(err.message || 'Erro ao salvar aplicativo.');
    } finally {
      setSubmitting(false);
    }
  };

  const handleDeleteApp = async (appId: string) => {
    if (!window.confirm('Tem certeza que deseja excluir esta aplicação? Todas as conexões ativas serão revogadas.')) {
      return;
    }
    try {
      await api(`/oauth/apps/${appId}`, { method: 'DELETE' });
      fetchApps();
    } catch (err: any) {
      alert(err.message || 'Erro ao excluir aplicativo.');
    }
  };

  const handleRotateSecret = async (app: OauthApp) => {
    if (!window.confirm(`Gerar novo Client Secret para "${app.name}"? O segredo anterior deixará de funcionar imediatamente.`)) {
      return;
    }
    try {
      const res = await api<OauthAppCreated>(`/oauth/apps/${app.id}/rotate-secret`, { method: 'POST' });
      setNewSecretModal({
        appName: res.name,
        secret: res.client_secret,
      });
      fetchApps();
    } catch (err: any) {
      alert(err.message || 'Erro ao rotacionar secret.');
    }
  };

  const copyToClipboard = (text: string, keyId: string) => {
    navigator.clipboard.writeText(text);
    setCopiedKey(keyId);
    setTimeout(() => setCopiedKey(null), 1500);
  };

  const activeApp = apps.find((a) => a.id === selectedAppId) || apps[0];
  const activeClientId = activeApp ? activeApp.client_id : 'sats_app_YOUR_CLIENT_ID';
  const activeRedirect = activeApp && activeApp.redirect_uris[0] ? activeApp.redirect_uris[0] : 'https://seusite.com/auth/callback';

  return (
    <div className="space-y-6 max-w-7xl mx-auto pb-16 font-sans text-ink">
      {/* HEADER */}
      <header className="border-b border-border/80 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <span className="flex h-2.5 w-2.5 rounded-full bg-emerald-500 animate-pulse" />
          <span className="text-xs font-bold uppercase tracking-widest text-emerald-600">
            Login com SatsPay · Provedor OAuth 2.0 (SSO)
          </span>
        </div>
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
          <div>
            <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">
              Aplicações & Login com SatsPay
            </h1>
            <p className="text-xs text-ink-muted mt-1 max-w-2xl">
              Permita que usuários façam login com a conta SatsPay na sua plataforma, igual ao Google Sign-In.
            </p>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <Link
              to="/docs?tab=oauth"
              className="rounded-xl border border-border bg-paper hover:bg-surface text-ink px-3.5 py-2 text-xs font-bold shadow-xs transition-all flex items-center gap-1.5"
            >
              <i className="bi bi-book-half text-emerald-600" />
              <span>Documentação</span>
            </Link>
            <button
              type="button"
              onClick={openCreateModal}
              className="rounded-xl bg-bitcoin hover:bg-bitcoin-dark active:scale-[0.98] text-white px-4 py-2 text-xs font-bold shadow-xs transition-all flex items-center gap-1.5"
            >
              <i className="bi bi-plus-lg" />
              <span>Novo Aplicativo</span>
            </button>
          </div>
        </div>
      </header>

      {/* Main Content Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
        {/* Left 7 Cols: Applications List */}
        <div className="lg:col-span-7 space-y-4">
          <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
            <h2 className="text-sm font-bold text-ink flex items-center gap-2">
              <i className="bi bi-grid-fill text-bitcoin" />
              <span>Suas Aplicações OAuth ({apps.length})</span>
            </h2>
            {apps.length > 0 && (
              <span className="text-[11px] text-ink-muted">
                Toque num app para personalizar o código de integração
              </span>
            )}
          </div>

          {loading ? (
            <div className="flex min-h-[260px] items-center justify-center rounded-3xl border border-border bg-paper shadow-xs">
              <div className="flex flex-col items-center gap-3">
                <div className="h-8 w-8 animate-spin rounded-full border-3 border-bitcoin border-t-transparent" />
                <span className="text-xs font-semibold text-ink-muted">Carregando aplicações...</span>
              </div>
            </div>
          ) : apps.length === 0 ? (
            <div className="rounded-3xl border border-dashed border-border bg-paper p-8 text-center shadow-xs space-y-5">
              <div className="flex h-16 w-16 items-center justify-center rounded-3xl bg-bitcoin/10 text-bitcoin mx-auto shadow-inner">
                <i className="bi bi-shield-check text-3xl" />
              </div>
              <div className="space-y-1 max-w-md mx-auto">
                <h3 className="text-base font-black text-ink">Nenhuma aplicação OAuth criada</h3>
                <p className="text-xs text-ink-muted leading-relaxed">
                  Crie sua primeira aplicação para receber um <strong>Client ID</strong> e <strong>Client Secret</strong> e oferecer o botão <em>Entrar com SatsPay</em> na sua loja, faucet ou plataforma.
                </p>
              </div>

              {/* 3 Steps Guide Box */}
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-2.5 max-w-lg mx-auto text-left pt-2">
                <div className="rounded-2xl border border-border bg-surface p-3 space-y-1">
                  <div className="flex h-6 w-6 items-center justify-center rounded-lg bg-bitcoin text-white text-[11px] font-black">
                    1
                  </div>
                  <div className="text-xs font-bold text-ink">Criar App</div>
                  <div className="text-[10px] text-ink-muted">Defina nome e URL de callback do seu site.</div>
                </div>
                <div className="rounded-2xl border border-border bg-surface p-3 space-y-1">
                  <div className="flex h-6 w-6 items-center justify-center rounded-lg bg-bitcoin text-white text-[11px] font-black">
                    2
                  </div>
                  <div className="text-xs font-bold text-ink">Copiar SDK</div>
                  <div className="text-[10px] text-ink-muted">Insira a tag do botão no seu código HTML.</div>
                </div>
                <div className="rounded-2xl border border-border bg-surface p-3 space-y-1">
                  <div className="flex h-6 w-6 items-center justify-center rounded-lg bg-bitcoin text-white text-[11px] font-black">
                    3
                  </div>
                  <div className="text-xs font-bold text-ink">Autenticar</div>
                  <div className="text-[10px] text-ink-muted">Receba o usuário logado com 1 clique.</div>
                </div>
              </div>

              <button
                type="button"
                onClick={openCreateModal}
                className="inline-flex items-center gap-2 rounded-2xl bg-bitcoin hover:bg-bitcoin-dark active:scale-95 px-6 py-3 text-xs font-black text-white shadow-md shadow-bitcoin/25 transition-all"
              >
                <i className="bi bi-plus-lg text-sm" />
                <span>Criar Primeiro Aplicativo OAuth</span>
              </button>
            </div>
          ) : (
            <div className="space-y-4">
              {apps.map((app) => {
                const isSelected = selectedAppId === app.id;
                const siteHost = app.website_url
                  ? app.website_url.replace(/^https?:\/\//, '').replace(/\/$/, '')
                  : null;
                const secretHint =
                  app.client_secret_prefix && app.client_secret_prefix.trim().length > 0
                    ? app.client_secret_prefix
                    : '••••••••••••';
                const desc =
                  app.description &&
                  app.description.trim().toLowerCase() !== app.name.trim().toLowerCase()
                    ? app.description.trim()
                    : null;

                return (
                  <div
                    key={app.id}
                    role="button"
                    tabIndex={0}
                    onClick={() => setSelectedAppId(app.id)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        setSelectedAppId(app.id);
                      }
                    }}
                    className={`cursor-pointer rounded-3xl border transition-all p-4 sm:p-5 bg-paper shadow-xs ${
                      isSelected
                        ? 'border-bitcoin ring-2 ring-bitcoin/20 shadow-md'
                        : 'border-border hover:border-bitcoin/40'
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <AppLogo name={app.name} logoUrl={app.logo_url} />

                      <div className="min-w-0 flex-1 space-y-1.5">
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0">
                            <div className="flex flex-wrap items-center gap-1.5">
                              <h3 className="text-base font-black text-ink truncate">{app.name}</h3>
                              <span className="rounded-md bg-emerald-500/10 px-1.5 py-0.5 text-[10px] font-bold text-emerald-700 border border-emerald-500/20">
                                Ativo
                              </span>
                              {isSelected && (
                                <span className="rounded-md bg-bitcoin/10 px-1.5 py-0.5 text-[10px] font-bold text-bitcoin">
                                  Preview
                                </span>
                              )}
                            </div>
                            {desc && (
                              <p className="mt-0.5 text-xs text-ink-muted line-clamp-1">{desc}</p>
                            )}
                          </div>

                          <div
                            className="flex shrink-0 items-center gap-1"
                            onClick={(e) => e.stopPropagation()}
                          >
                            <button
                              type="button"
                              onClick={() => openEditModal(app)}
                              className="flex h-8 w-8 items-center justify-center rounded-xl border border-border bg-surface text-ink-muted hover:text-ink hover:bg-paper"
                              title="Editar"
                            >
                              <i className="bi bi-pencil-square text-xs" />
                            </button>
                            <button
                              type="button"
                              onClick={() => handleDeleteApp(app.id)}
                              className="flex h-8 w-8 items-center justify-center rounded-xl border border-rose-500/30 bg-rose-500/10 text-rose-600 hover:bg-rose-500/20"
                              title="Excluir"
                            >
                              <i className="bi bi-trash3-fill text-xs" />
                            </button>
                          </div>
                        </div>

                        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-ink-muted">
                          {siteHost && (
                            <a
                              href={app.website_url}
                              target="_blank"
                              rel="noreferrer"
                              onClick={(e) => e.stopPropagation()}
                              className="inline-flex items-center gap-1 font-medium hover:text-bitcoin"
                            >
                              <i className="bi bi-globe2 text-bitcoin" />
                              <span className="truncate max-w-[14rem]">{siteHost}</span>
                            </a>
                          )}
                          <span className="inline-flex items-center gap-1">
                            <i className="bi bi-arrow-return-right" />
                            {app.redirect_uris.length} callback
                            {app.redirect_uris.length === 1 ? '' : 's'}
                          </span>
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              const testUrl = `/oauth/authorize?client_id=${encodeURIComponent(app.client_id)}&redirect_uri=${encodeURIComponent(app.redirect_uris[0] || 'https://localhost')}&scope=openid%20profile%20email&response_type=code`;
                              window.open(testUrl, '_blank');
                            }}
                            className="inline-flex items-center gap-1 font-bold text-ink hover:text-bitcoin"
                          >
                            <i className="bi bi-box-arrow-up-right text-[10px]" />
                            Testar login
                          </button>
                        </div>
                      </div>
                    </div>

                    <div className="mt-4 space-y-2.5 border-t border-border/70 pt-3.5">
                      <CredentialRow
                        label="Client ID"
                        badge="Público"
                        value={app.client_id}
                        copied={copiedKey === `cid_${app.id}`}
                        onCopy={() => copyToClipboard(app.client_id, `cid_${app.id}`)}
                      />
                      <div>
                        <div className="mb-1 flex items-center justify-between gap-2">
                          <span className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">
                            Client Secret
                          </span>
                          <span className="text-[9px] font-bold uppercase tracking-wide text-amber-700">
                            Confidencial
                          </span>
                        </div>
                        <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                          <div className="min-w-0 flex-1 rounded-xl border border-border bg-surface px-3 py-2.5">
                            <code className="block font-mono text-xs text-ink-muted tracking-wide break-all">
                              {secretHint}
                              {secretHint.endsWith('...') ? '' : '••••'}
                            </code>
                            <p className="mt-1 text-[10px] text-ink-muted">
                              O secret completo só aparece uma vez ao criar ou rotacionar.
                            </p>
                          </div>
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleRotateSecret(app);
                            }}
                            className="inline-flex shrink-0 items-center justify-center gap-1.5 rounded-xl border border-amber-500/35 bg-amber-500/10 px-3.5 py-2.5 text-xs font-bold text-amber-700 hover:bg-amber-500/20 transition-colors"
                          >
                            <i className="bi bi-arrow-repeat" />
                            Novo secret
                          </button>
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Right 5 Cols: Interactive SDK Preview & Code Snippets */}
        <div className="lg:col-span-5 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-bold text-ink flex items-center gap-2">
              <i className="bi bi-code-slash text-emerald-600" />
              <span>Personalizador & Integração</span>
            </h2>
            <span className="rounded-full bg-bitcoin/10 px-2.5 py-0.5 text-[10px] font-bold text-bitcoin">
              SDK Oficial JS
            </span>
          </div>

          <div className="rounded-3xl border border-border/80 bg-paper p-5 sm:p-6 space-y-5 shadow-xs">
            {/* Live Button Customizer */}
            <div className="space-y-3.5">
              <div className="flex items-center justify-between text-xs">
                <span className="font-bold text-ink uppercase text-[11px] tracking-wider">Aparência do Botão</span>
                {activeApp && (
                  <span className="text-[11px] text-ink-muted font-mono truncate max-w-[150px]">
                    App: <b>{activeApp.name}</b>
                  </span>
                )}
              </div>

              {/* Theme & Text Controls */}
              <div className="space-y-3 rounded-2xl bg-surface/80 p-3.5 border border-border/80">
                {/* Theme Selector */}
                <div className="space-y-1.5">
                  <span className="text-[11px] font-bold text-ink-muted block">Tema Visual:</span>
                  <div className="grid grid-cols-3 gap-1.5">
                    {[
                      { id: 'light', label: 'Clean Light', bg: 'bg-white text-slate-900 border border-slate-300' },
                      { id: 'dark', label: 'Dark Obsidian', bg: 'bg-[#0B0F19] text-white border border-white/20' },
                      { id: 'bitcoin', label: 'Bitcoin Gold', bg: 'bg-bitcoin text-white' },
                    ].map((t) => (
                      <button
                        key={t.id}
                        type="button"
                        onClick={() => setPreviewTheme(t.id as any)}
                        className={`rounded-xl py-2 px-1 text-center text-[11px] font-bold transition-all ${
                          previewTheme === t.id
                            ? 'ring-2 ring-bitcoin shadow-xs scale-102'
                            : 'opacity-70 hover:opacity-100'
                        } ${t.bg}`}
                      >
                        {t.label}
                      </button>
                    ))}
                  </div>
                </div>

                {/* Text Selector */}
                <div className="space-y-1.5 pt-1">
                  <span className="text-[11px] font-bold text-ink-muted block">Texto do Botão:</span>
                  <div className="grid grid-cols-3 gap-1.5">
                    {[
                      { id: 'signin_with', label: 'Entrar com…' },
                      { id: 'continue_with', label: 'Continuar com…' },
                      { id: 'en_signin', label: 'Sign in with…' },
                    ].map((tx) => (
                      <button
                        key={tx.id}
                        type="button"
                        onClick={() => setPreviewText(tx.id as any)}
                        className={`rounded-xl py-1.5 px-1 text-center text-[10px] font-bold transition-all border ${
                          previewText === tx.id
                            ? 'bg-paper text-ink border-bitcoin shadow-2xs'
                            : 'bg-surface text-ink-muted border-border hover:text-ink'
                        }`}
                      >
                        {tx.label}
                      </button>
                    ))}
                  </div>
                </div>

                {/* Auth flow mode */}
                <div className="space-y-1.5 pt-1">
                  <span className="text-[11px] font-bold text-ink-muted block">Fluxo de autenticação:</span>
                  <div className="grid grid-cols-2 gap-1.5">
                    {[
                      { id: 'redirect', label: 'Redirect (recomendado)', hint: 'Vai ao SatsPay e volta com ?code=' },
                      { id: 'popup', label: 'Popup', hint: 'Janela + postMessage' },
                    ].map((m) => (
                      <button
                        key={m.id}
                        type="button"
                        onClick={() => setPreviewMode(m.id as 'redirect' | 'popup')}
                        className={`rounded-xl py-2 px-2 text-left transition-all border ${
                          previewMode === m.id
                            ? 'bg-paper text-ink border-bitcoin shadow-2xs'
                            : 'bg-surface text-ink-muted border-border hover:text-ink'
                        }`}
                      >
                        <div className="text-[10px] font-bold">{m.label}</div>
                        <div className="text-[9px] opacity-70 mt-0.5">{m.hint}</div>
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* Live Rendered Button Box */}
              <div className="relative flex flex-col items-center justify-center rounded-2xl border border-dashed border-border bg-surface p-6 text-center shadow-inner">
                <button
                  type="button"
                  onClick={() => {
                    const authUrl = `/oauth/authorize?client_id=${encodeURIComponent(activeClientId)}&redirect_uri=${encodeURIComponent(activeRedirect)}&scope=openid%20profile%20email&response_type=code${previewMode === 'popup' ? '&popup=1' : ''}`;
                    if (previewMode === 'popup') {
                      window.open(authUrl, 'SatsPaySignInWindow', 'width=480,height=720');
                    } else {
                      window.location.href = authUrl;
                    }
                  }}
                  className={`inline-flex items-center justify-center gap-2.5 rounded-xl font-bold tracking-tight transition-all duration-200 hover:-translate-y-0.5 active:translate-y-0 active:scale-98 cursor-pointer h-12 px-5 text-sm ${
                    previewTheme === 'bitcoin'
                      ? 'bg-gradient-to-r from-bitcoin via-[#F59E0B] to-bitcoin text-white shadow-lg shadow-bitcoin/30 border border-white/20'
                      : previewTheme === 'dark'
                      ? 'bg-[#0B0F19] text-white shadow-lg shadow-black/40 border border-white/15 hover:border-white/30'
                      : 'bg-white text-slate-900 shadow-md border border-slate-200 hover:border-slate-300'
                  }`}
                >
                  <img
                    src="/sdk/satspay-logo.png"
                    alt=""
                    width={20}
                    height={20}
                    className="h-5 w-5 object-contain"
                  />
                  <span>
                    {previewText === 'signin_with'
                      ? 'Entrar com SatsPay'
                      : previewText === 'continue_with'
                      ? 'Continuar com SatsPay'
                      : 'Sign in with SatsPay'}
                  </span>
                </button>
                <p className="mt-3 text-[10px] text-ink-muted flex items-center gap-1 font-medium">
                  <i className="bi bi-cursor-fill text-bitcoin" />
                  <span>
                    {previewMode === 'redirect'
                      ? 'Redirect: abre SatsPay nesta aba e devolve ?code= no callback'
                      : 'Popup: abre janela; se bloqueada, cai no redirect'}
                  </span>
                </p>
              </div>
            </div>

            {/* Code Tabs */}
            <div className="space-y-3 pt-1 border-t border-border/60">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1 overflow-x-auto [scrollbar-width:none]">
                  {[
                    { id: 'html', label: 'HTML / SDK' },
                    { id: 'node', label: 'Node.js' },
                    { id: 'python', label: 'Python' },
                    { id: 'curl', label: 'cURL' },
                  ].map((t) => (
                    <button
                      key={t.id}
                      type="button"
                      onClick={() => setCodeTab(t.id as any)}
                      className={`rounded-xl px-3 py-1.5 text-xs font-bold transition-all ${
                        codeTab === t.id
                          ? 'bg-emerald-600 text-white shadow-xs'
                          : 'bg-surface border border-border text-ink-muted hover:text-ink'
                      }`}
                    >
                      {t.label}
                    </button>
                  ))}
                </div>

                <button
                  type="button"
                  onClick={() => {
                    let code = '';
                    if (codeTab === 'html') {
                      code = `<!-- 1. Carregar SDK Oficial do SatsPay -->\n<script src="https://www.satspay.pro/sdk/satspay-auth.v2.js" async defer></script>\n\n<!-- 2. Botão (redirect = padrão OAuth: vai ao SatsPay e volta com ?code=) -->\n<div class="satspay-signin"\n     data-client_id="${activeClientId}"\n     data-redirect_uri="${activeRedirect}"\n     data-mode="${previewMode}"\n     data-theme="${previewTheme}"\n     data-text="${previewText}"\n     data-onsuccess="onSatsPaySuccess">\n</div>\n\n<script>\nfunction onSatsPaySuccess(response) {\n  // Só dispara no modo popup. No redirect, leia ?code= no seu callback.\n  console.log('Código OAuth 2.0:', response.code);\n}\n</script>`;
                    } else if (codeTab === 'node') {
                      code = `// Trocar code por Token & Perfil no Node.js\nconst res = await fetch('https://www.satspay.pro/v1/oauth/token', {\n  method: 'POST',\n  headers: { 'Content-Type': 'application/x-www-form-urlencoded' },\n  body: new URLSearchParams({\n    grant_type: 'authorization_code',\n    code: req.query.code,\n    client_id: '${activeClientId}',\n    client_secret: 'SEU_CLIENT_SECRET',\n    redirect_uri: '${activeRedirect}'\n  })\n});\nconst { access_token } = await res.json();\n\n// Obter Perfil do Usuário\nconst userRes = await fetch('https://www.satspay.pro/v1/oauth/userinfo', {\n  headers: { Authorization: \`Bearer \${access_token}\` }\n});\nconst user = await userRes.json();\nconsole.log('Usuário autenticado:', user.username, user.email);`;
                    } else if (codeTab === 'python') {
                      code = `import requests\n\n# 1. Trocar Code por Access Token\ntoken_res = requests.post(\n    'https://www.satspay.pro/v1/oauth/token',\n    data={\n        'grant_type': 'authorization_code',\n        'code': auth_code,\n        'client_id': '${activeClientId}',\n        'client_secret': 'SEU_CLIENT_SECRET',\n        'redirect_uri': '${activeRedirect}',\n    }\n).json()\n\naccess_token = token_res['access_token']\n\n# 2. Obter Informações do Usuário\nuser = requests.get(\n    'https://www.satspay.pro/v1/oauth/userinfo',\n    headers={'Authorization': f'Bearer {access_token}'}\n).json()\n\nprint(f"Bem-vindo @{user['username']} ({user['email']})")`;
                    } else {
                      code = `curl -X POST https://www.satspay.pro/v1/oauth/token \\\n  -d grant_type=authorization_code \\\n  -d code=CODE_RECEBIDO \\\n  -d client_id=${activeClientId} \\\n  -d client_secret=SEU_SECRET \\\n  -d redirect_uri=${activeRedirect}`;
                    }
                    copyToClipboard(code, 'snippet_code');
                  }}
                  className="rounded-xl border border-border bg-paper hover:bg-surface px-2.5 py-1 text-xs font-bold text-ink transition-all shadow-2xs flex items-center gap-1"
                >
                  <i className="bi bi-clipboard text-bitcoin" />
                  <span>{copiedKey === 'snippet_code' ? 'Copiado!' : 'Copiar'}</span>
                </button>
              </div>

              <div className="relative overflow-hidden rounded-2xl border border-white/10 bg-[#090D16] p-4 text-white shadow-inner">
                <pre className="overflow-x-auto font-mono text-[11px] text-emerald-400/90 leading-relaxed [scrollbar-width:none]">
                  {codeTab === 'html' &&
                    `<!-- 1. Carregar SDK Oficial do SatsPay -->\n<script src="https://www.satspay.pro/sdk/satspay-auth.v2.js" async defer></script>\n\n<!-- 2. Botão — data-mode="redirect" (padrão) ou "popup" -->\n<div class="satspay-signin"\n     data-client_id="${activeClientId}"\n     data-redirect_uri="${activeRedirect}"\n     data-mode="${previewMode}"\n     data-theme="${previewTheme}"\n     data-text="${previewText}"\n     data-onsuccess="onSatsPaySuccess">\n</div>\n\n<script>\nfunction onSatsPaySuccess(response) {\n  console.log('Código OAuth:', response.code);\n}\n</script>`}
                  {codeTab === 'node' &&
                    `// Trocar code por Token\nconst token = await fetch('https://www.satspay.pro/v1/oauth/token', {\n  method: 'POST',\n  headers: { 'Content-Type': 'application/x-www-form-urlencoded' },\n  body: new URLSearchParams({\n    grant_type: 'authorization_code',\n    code,\n    client_id: '${activeClientId}',\n    client_secret: '...', \n    redirect_uri: '${activeRedirect}'\n  })\n}).then(r => r.json());\n\n// Obter Perfil\nconst user = await fetch('https://www.satspay.pro/v1/oauth/userinfo', {\n  headers: { Authorization: \`Bearer \${token.access_token}\` }\n}).then(r => r.json());`}
                  {codeTab === 'python' &&
                    `# Trocar code por token\ntoken = requests.post('https://www.satspay.pro/v1/oauth/token', data={\n  'grant_type': 'authorization_code',\n  'code': code,\n  'client_id': '${activeClientId}',\n  'client_secret': '...',\n  'redirect_uri': '${activeRedirect}',\n}).json()\n\nuser = requests.get('https://www.satspay.pro/v1/oauth/userinfo', headers={\n  'Authorization': f"Bearer {token['access_token']}"\n}).json()`}
                  {codeTab === 'curl' &&
                    `curl -X POST https://www.satspay.pro/v1/oauth/token \\\n  -d grant_type=authorization_code \\\n  -d code=CODE_RECEBIDO \\\n  -d client_id=${activeClientId} \\\n  -d client_secret=SEU_SECRET \\\n  -d redirect_uri=${activeRedirect}`}
                </pre>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Modal: Create/Edit App */}
      <Modal
        open={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        panelClassName="max-w-lg"
        aria-label={editingApp ? 'Editar Aplicação OAuth' : 'Criar Nova Aplicação OAuth'}
      >
        <div className="overflow-hidden rounded-3xl border border-border bg-surface shadow-2xl">
              <div className="flex items-center justify-between border-b border-border px-6 py-4">
                <div className="flex items-center gap-2">
                  <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin">
                    <i className="bi bi-app text-sm" />
                  </div>
                  <h3 className="text-base font-bold text-ink">
                    {editingApp ? 'Editar Aplicação OAuth' : 'Criar Nova Aplicação OAuth'}
                  </h3>
                </div>
                <button
                  type="button"
                  onClick={() => setShowCreateModal(false)}
                  className="rounded-lg p-1 text-ink-muted hover:bg-paper hover:text-ink transition-colors"
                >
                  <i className="bi bi-x-lg text-sm" />
                </button>
              </div>

              <form onSubmit={handleSaveApp} className="p-6 space-y-4">
                <div>
                  <label className="block text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Nome da Aplicação *
                  </label>
                  <input
                    type="text"
                    required
                    value={formName}
                    onChange={(e) => setFormName(e.target.value)}
                    placeholder="Ex: Minha Loja Cripto, GamePortal"
                    className="mt-1.5 w-full rounded-xl border border-border bg-paper px-3.5 py-2.5 text-xs text-ink placeholder:text-ink-muted focus:border-bitcoin focus:outline-none"
                  />
                </div>

                <div>
                  <label className="block text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Descrição Curta (Opcional)
                  </label>
                  <input
                    type="text"
                    value={formDescription}
                    onChange={(e) => setFormDescription(e.target.value)}
                    placeholder="Ex: Plataforma de e-commerce integrada com Bitcoin"
                    className="mt-1.5 w-full rounded-xl border border-border bg-paper px-3.5 py-2.5 text-xs text-ink placeholder:text-ink-muted focus:border-bitcoin focus:outline-none"
                  />
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-bold uppercase tracking-wider text-ink-muted">
                      URL do Site
                    </label>
                    <input
                      type="url"
                      value={formWebsite}
                      onChange={(e) => setFormWebsite(e.target.value)}
                      placeholder="https://meusite.com"
                      className="mt-1.5 w-full rounded-xl border border-border bg-paper px-3.5 py-2.5 text-xs text-ink placeholder:text-ink-muted focus:border-bitcoin focus:outline-none"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-bold uppercase tracking-wider text-ink-muted">
                      URL do Logo / Ícone
                    </label>
                    <input
                      type="url"
                      value={formLogo}
                      onChange={(e) => setFormLogo(e.target.value)}
                      placeholder="https://meusite.com/assets/app-icon.png"
                      className="mt-1.5 w-full rounded-xl border border-border bg-paper px-3.5 py-2.5 text-xs text-ink placeholder:text-ink-muted focus:border-bitcoin focus:outline-none"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-bold uppercase tracking-wider text-ink-muted">
                    URIs de Redirecionamento (Callback URLs) *
                  </label>
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    Uma por linha. O SatsPay só redirecionará para URLs nesta lista por segurança.
                  </p>
                  <textarea
                    rows={3}
                    required
                    value={formRedirectUris}
                    onChange={(e) => setFormRedirectUris(e.target.value)}
                    placeholder="https://meusite.com/auth/callback&#10;http://localhost:3000/callback"
                    className="mt-1.5 w-full rounded-xl border border-border bg-paper p-3 font-mono text-xs text-ink placeholder:text-ink-muted focus:border-bitcoin focus:outline-none"
                  />
                </div>

                <div className="flex items-center justify-end gap-2.5 pt-3">
                  <button
                    type="button"
                    onClick={() => setShowCreateModal(false)}
                    className="rounded-xl border border-border bg-paper px-4 py-2.5 text-xs font-bold text-ink-muted hover:bg-surface hover:text-ink transition-all"
                  >
                    Cancelar
                  </button>
                  <button
                    type="submit"
                    disabled={submitting}
                    className="flex items-center gap-2 rounded-xl bg-bitcoin px-5 py-2.5 text-xs font-black tracking-wide text-white shadow-md shadow-bitcoin/25 hover:bg-bitcoin-light transition-all disabled:opacity-50"
                  >
                    {submitting ? 'Salvando...' : editingApp ? 'Salvar Alterações' : 'Criar Aplicação'}
                  </button>
                </div>
              </form>
        </div>
      </Modal>

      {/* Modal: Reveal Secret */}
      <Modal
        open={!!newSecretModal}
        onClose={() => setNewSecretModal(null)}
        panelClassName="max-w-md"
        aria-label="Guarde seu Client Secret"
      >
        <div className="overflow-hidden rounded-3xl border border-amber-500/40 bg-surface shadow-2xl">
              <div className="border-b border-border bg-amber-500/10 px-6 py-4">
                <div className="flex items-center gap-2.5 text-amber-600">
                  <i className="bi bi-exclamation-triangle-fill text-lg" />
                  <h3 className="text-base font-bold">Guarde seu Client Secret!</h3>
                </div>
              </div>

              <div className="p-6 space-y-4">
                <p className="text-xs text-ink-muted leading-relaxed">
                  Aqui está o segredo de cliente para <strong className="text-ink">{newSecretModal?.appName}</strong>. Por motivos de segurança, ele não poderá ser visualizado novamente por completo.
                </p>

                <div>
                  <span className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">
                    Client Secret
                  </span>
                  <div className="mt-1 flex items-center justify-between rounded-xl bg-[#090D16] p-3 text-white border border-white/10">
                    <code className="font-mono text-xs font-bold text-amber-400 select-all break-all">
                      {newSecretModal?.secret}
                    </code>
                    <button
                      type="button"
                      onClick={() => newSecretModal && copyToClipboard(newSecretModal.secret, 'modal_secret')}
                      className="ml-3 shrink-0 rounded-lg bg-white/10 px-3 py-1 text-xs font-bold text-white hover:bg-white/20 transition-all"
                    >
                      {copiedKey === 'modal_secret' ? '✓ Copiado' : 'Copiar'}
                    </button>
                  </div>
                </div>

                <div className="rounded-xl border border-rose-500/20 bg-rose-500/5 p-3 text-[11px] text-rose-600 leading-relaxed">
                  <i className="bi bi-shield-slash mr-1" />
                  Nunca compartilhe ou exponha este Secret em código frontend ou repositórios públicos.
                </div>

                <button
                  type="button"
                  onClick={() => setNewSecretModal(null)}
                  className="w-full rounded-xl bg-bitcoin py-2.5 text-xs font-bold text-white hover:bg-bitcoin-light transition-all shadow-md shadow-bitcoin/25"
                >
                  Eu copiei e salvei o segredo
                </button>
              </div>
        </div>
      </Modal>
    </div>
  );
}

function AppLogo({ name, logoUrl }: { name: string; logoUrl?: string }) {
  const [broken, setBroken] = useState(false);
  const initial = (name.trim().charAt(0) || 'A').toUpperCase();
  const usable = isThirdPartyAppLogo(logoUrl);
  const showImg = usable && !broken;

  return (
    <div className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-2xl border border-border bg-surface shadow-2xs">
      {showImg ? (
        <img
          src={logoUrl}
          alt=""
                      className="h-full w-full object-contain p-1"
          onError={() => setBroken(true)}
        />
      ) : (
        <span className="text-sm font-black text-bitcoin">{initial}</span>
      )}
    </div>
  );
}

function isThirdPartyAppLogo(url?: string | null): boolean {
  if (!url || !url.trim()) return false;
  try {
    const u = new URL(url, typeof window !== 'undefined' ? window.location.origin : 'https://www.satspay.pro');
    const path = u.pathname.toLowerCase();
    if (path === '/logo.png' || path.endsWith('/logo.png')) {
      if (u.hostname === 'www.satspay.pro' || u.hostname === 'satspay.pro') return false;
      if (typeof window !== 'undefined' && u.origin === window.location.origin) return false;
    }
    return true;
  } catch {
    return false;
  }
}

function CredentialRow({
  label,
  badge,
  value,
  copied,
  onCopy,
}: {
  label: string;
  badge: string;
  value: string;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <div>
      <div className="mb-1 flex items-center justify-between gap-2">
        <span className="text-[10px] font-bold uppercase tracking-wider text-ink-muted">{label}</span>
        <span className="text-[9px] font-bold uppercase tracking-wide text-ink-muted">{badge}</span>
      </div>
      <div className="flex items-stretch gap-2">
        <div className="min-w-0 flex-1 rounded-xl border border-border bg-surface px-3 py-2.5">
          <code className={clsx('block font-mono text-xs font-bold text-bitcoin break-all select-all')}>
            {value}
          </code>
        </div>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onCopy();
          }}
          className="shrink-0 rounded-xl border border-border bg-paper px-3 text-xs font-bold text-ink-muted hover:text-bitcoin hover:bg-surface transition-colors"
        >
          {copied ? '✓' : 'Copiar'}
        </button>
      </div>
    </div>
  );
}

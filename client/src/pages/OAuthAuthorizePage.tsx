import { useEffect, useState } from 'react';
import { useSearchParams, useNavigate, Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../stores/auth.js';
import { api } from '../lib/api.js';
import { reportAuthFailure } from '../lib/reportError.js';

interface AuthorizeInfo {
  app_id: string;
  app_name: string;
  description?: string;
  website_url?: string;
  logo_url?: string;
  redirect_uri: string;
  scopes: string[];
  user?: {
    id: string;
    username: string;
    email: string;
  };
}

export function OAuthAuthorizePage() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const user = useAuthStore((s) => s.user);

  const clientId = searchParams.get('client_id') || '';
  const redirectUri = searchParams.get('redirect_uri') || '';
  const scope = searchParams.get('scope') || 'openid profile email';
  const state = searchParams.get('state') || '';
  const popupParam = searchParams.get('popup') === '1';
  const codeChallenge = searchParams.get('code_challenge') || '';
  const codeChallengeMethod = searchParams.get('code_challenge_method') || 'S256';

  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [appInfo, setAppInfo] = useState<AuthorizeInfo | null>(null);
  const [logoBroken, setLogoBroken] = useState(false);

  // Preserve PKCE + popup across login round-trips.
  useEffect(() => {
    if (popupParam) {
      try {
        sessionStorage.setItem('satspay_oauth_popup', '1');
      } catch {
        // ignore
      }
    }
    if (codeChallenge) {
      try {
        sessionStorage.setItem(
          'satspay_authorize_pkce',
          JSON.stringify({ code_challenge: codeChallenge, code_challenge_method: codeChallengeMethod }),
        );
      } catch {
        // ignore
      }
    }
  }, [popupParam, codeChallenge, codeChallengeMethod]);

  const isPopup = (() => {
    if (popupParam) return true;
    try {
      if (sessionStorage.getItem('satspay_oauth_popup') === '1') return true;
    } catch {
      // ignore
    }
    if (typeof window !== 'undefined' && window.name === 'SatsPaySignInWindow') return true;
    try {
      if (typeof window !== 'undefined' && window.opener && !window.opener.closed) return true;
    } catch {
      // ignore
    }
    return false;
  })();

  useEffect(() => {
    if (!clientId) {
      setError('client_id ausente na requisição OAuth.');
      setLoading(false);
      return;
    }

    let query = `client_id=${encodeURIComponent(clientId)}`;
    if (redirectUri) query += `&redirect_uri=${encodeURIComponent(redirectUri)}`;
    if (scope) query += `&scope=${encodeURIComponent(scope)}`;

    api<AuthorizeInfo>(`/oauth/authorize/info?${query}`)
      .then((data) => {
        setAppInfo(data);
        setLogoBroken(false);
        setLoading(false);
      })
      .catch((err: any) => {
        const msg = err.message || 'Falha ao carregar dados do aplicativo.';
        setError(msg);
        setLoading(false);
        reportAuthFailure('oauth', `Authorize info failed: ${msg}`, { clientId });
      });
  }, [clientId, redirectUri, scope]);

  const finishOAuth = (redirectUrl: string, decision: 'approve' | 'deny') => {
    try {
      sessionStorage.removeItem('satspay_oauth_popup');
    } catch {
      // ignore
    }

    // Popup / COOP-safe path: go through bridge (postMessage if opener exists, else merchant redirect).
    if (isPopup) {
      const bridge = new URL('/oauth/bridge', window.location.origin);
      bridge.searchParams.set('redirect_url', redirectUrl);
      if (decision === 'deny') bridge.searchParams.set('error', 'access_denied');
      window.location.replace(bridge.toString());
      return;
    }

    // Standard OAuth redirect back to merchant callback
    window.location.href = redirectUrl;
  };

  const handleDecision = async (decision: 'approve' | 'deny') => {
    if (!appInfo) return;
    setSubmitting(true);
    setError(null);

    let challenge = codeChallenge;
    let method = codeChallengeMethod;
    if (!challenge) {
      try {
        const raw = sessionStorage.getItem('satspay_authorize_pkce');
        if (raw) {
          const parsed = JSON.parse(raw) as { code_challenge?: string; code_challenge_method?: string };
          challenge = parsed.code_challenge || '';
          method = parsed.code_challenge_method || 'S256';
        }
      } catch {
        // ignore
      }
    }

    try {
      const res = await api<{ redirect_url: string }>('/oauth/authorize', {
        method: 'POST',
        json: {
          client_id: clientId,
          redirect_uri: appInfo.redirect_uri,
          scope,
          state,
          decision,
          ...(challenge
            ? { code_challenge: challenge, code_challenge_method: method }
            : {}),
        },
      });

      try {
        sessionStorage.removeItem('satspay_authorize_pkce');
      } catch {
        // ignore
      }

      finishOAuth(res.redirect_url, decision);
    } catch (err: any) {
      const msg = err.message || 'Erro ao processar autorização.';
      setError(msg);
      setSubmitting(false);
      reportAuthFailure('oauth', `Authorize decision failed: ${msg}`, {
        clientId,
        decision,
      });
    }
  };

  const returnTo = (() => {
    const url = new URL(window.location.href);
    if (isPopup) url.searchParams.set('popup', '1');
    return encodeURIComponent(url.pathname + url.search);
  })();
  const siteHost = appInfo?.website_url
    ? appInfo.website_url.replace(/^https?:\/\//, '').replace(/\/$/, '')
    : null;
  const appInitial = (appInfo?.app_name?.trim().charAt(0) || 'A').toUpperCase();
  const thirdPartyLogo = isThirdPartyAppLogo(appInfo?.logo_url) ? appInfo!.logo_url : undefined;
  const faviconLogo =
    !thirdPartyLogo && appInfo?.website_url ? websiteFaviconUrl(appInfo.website_url) : undefined;
  const appLogoSrc = thirdPartyLogo || faviconLogo;
  const showAppLogo = Boolean(appLogoSrc) && !logoBroken;

  if (loading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas p-4">
        <div className="flex flex-col items-center gap-3">
          <div className="h-9 w-9 animate-spin rounded-full border-[3px] border-bitcoin border-t-transparent" />
          <p className="text-sm text-ink-muted">Carregando autorização…</p>
        </div>
      </div>
    );
  }

  if (error && !appInfo) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas p-4">
        <div className="w-full max-w-md rounded-2xl border border-border bg-paper p-6 text-center shadow-sm">
          <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-rose-50 text-rose-600">
            <i className="bi bi-shield-x text-xl" />
          </div>
          <h2 className="text-lg font-bold text-ink">Erro de autorização</h2>
          <p className="mt-2 text-sm text-rose-600">{error}</p>
          <Link
            to="/dashboard"
            className="mt-6 inline-flex rounded-xl bg-bitcoin px-5 py-2.5 text-sm font-semibold text-white hover:bg-bitcoin-light"
          >
            Ir para o Dashboard
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen flex-col bg-[#FAFBFC] text-ink">
      <header className="flex items-center justify-center px-4 py-5">
        <Link to="/welcome" className="inline-flex items-center gap-2.5">
          <img src="/sdk/satspay-logo.png" alt="" className="h-7 w-7 object-contain" />
          <span className="text-[15px] font-bold tracking-tight">SatsPay</span>
        </Link>
      </header>

      <main className="flex flex-1 items-start justify-center px-4 pb-10 pt-2 sm:pt-6">
        <div className="w-full max-w-[420px]">
          <div className="rounded-[22px] border border-border/90 bg-paper p-6 sm:p-7 shadow-[0_1px_2px_rgba(15,23,42,0.04),0_12px_32px_rgba(15,23,42,0.06)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.35),0_12px_32px_rgba(0,0,0,0.45)]">
            {/* Logos cruas — sem card/caixa em volta */}
            <div className="flex flex-col items-center text-center">
              <div className="flex items-center justify-center gap-5 sm:gap-6">
                <img
                  src="/sdk/satspay-logo.png"
                  alt="SatsPay"
                  className="h-14 w-14 object-contain sm:h-16 sm:w-16"
                />
                <i className="bi bi-arrow-left-right text-base text-ink-muted/50" aria-hidden />
                {showAppLogo ? (
                  <img
                    src={appLogoSrc}
                    alt={appInfo?.app_name || 'App'}
                    className="h-14 w-14 object-contain sm:h-16 sm:w-16"
                    onError={() => setLogoBroken(true)}
                  />
                ) : (
                  <span className="flex h-14 w-14 items-center justify-center text-2xl font-black text-bitcoin sm:h-16 sm:w-16">
                    {appInitial}
                  </span>
                )}
              </div>

              <h1 className="mt-5 text-[1.35rem] font-bold leading-snug tracking-tight text-ink sm:text-[1.45rem]">
                <span className="text-bitcoin">{appInfo?.app_name}</span>
                <span className="font-semibold text-ink"> quer acessar sua conta</span>
              </h1>
              <p className="mt-2 text-[12px] leading-relaxed text-ink-muted">
                {siteHost ? (
                  <a
                    href={appInfo!.website_url}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1.5 font-medium text-ink-muted transition hover:text-bitcoin"
                  >
                    <span>{siteHost}</span>
                    <i className="bi bi-box-arrow-up-right text-[9px] opacity-70" />
                  </a>
                ) : (
                  'Aplicativo de terceiros via SatsPay Identity'
                )}
              </p>
            </div>

            <div className="mt-6 space-y-4">
              {error && (
                <div className="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2.5 text-xs text-rose-700">
                  {error}
                </div>
              )}

              {user ? (
                <div className="flex items-center justify-between gap-3 rounded-2xl border border-border bg-gradient-to-b from-surface to-white px-3.5 py-3.5">
                  <div className="flex min-w-0 items-center gap-3">
                    <div className="relative flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-bitcoin text-sm font-black text-white shadow-sm shadow-bitcoin/25">
                      {user.username?.charAt(0)?.toUpperCase() || 'U'}
                      <span className="absolute -bottom-0.5 -right-0.5 flex h-3.5 w-3.5 items-center justify-center rounded-full border-2 border-white bg-emerald-500">
                        <i className="bi bi-check text-[8px] text-white" />
                      </span>
                    </div>
                    <div className="min-w-0 text-left">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate text-[13px] font-bold text-ink">@{user.username}</span>
                        <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[9px] font-bold uppercase tracking-wide text-emerald-700 ring-1 ring-emerald-100">
                          Conectado
                        </span>
                      </div>
                      <span className="mt-0.5 block truncate text-[11px] text-ink-muted">{user.email}</span>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => navigate(`/login?return_to=${returnTo}`)}
                    className="shrink-0 rounded-lg px-2 py-1.5 text-[11px] font-bold text-bitcoin transition hover:bg-bitcoin/10"
                  >
                    Trocar
                  </button>
                </div>
              ) : (
                <div className="rounded-2xl border border-amber-200 bg-amber-50 p-4 text-center">
                  <p className="text-xs text-amber-900">
                    {t('oauth.authorize.needLogin')}
                  </p>
                  <div className="mt-3 flex gap-2">
                    <Link
                      to={`/login?return_to=${returnTo}`}
                      className="flex-1 rounded-xl bg-bitcoin py-2.5 text-xs font-bold text-white hover:bg-bitcoin-light"
                    >
                      {t('common.signIn')}
                    </Link>
                    <Link
                      to={`/register?return_to=${returnTo}`}
                      className="flex-1 rounded-xl border border-border bg-paper py-2.5 text-xs font-bold text-ink hover:bg-surface"
                    >
                      {t('common.signUp')}
                    </Link>
                  </div>
                </div>
              )}

              <div>
                <h2 className="text-[11px] font-bold uppercase tracking-wider text-ink-muted">
                  {t('oauth.authorize.permissions')}
                </h2>
                <ul className="mt-2.5 space-y-2">
                  <PermissionItem
                    icon="bi-person-check"
                    title={t('oauth.authorize.identityTitle')}
                    body={t('oauth.authorize.identityBody')}
                  />
                  <PermissionItem
                    icon="bi-envelope-check"
                    title={t('oauth.authorize.emailTitle')}
                    body={t('oauth.authorize.emailBody', {
                      email: user?.email || '…',
                    })}
                  />
                  <PermissionItem
                    icon="bi-shield-check"
                    title="Autenticação segura (SSO)"
                    body="Fazer login automático sem compartilhar sua senha."
                  />
                </ul>
              </div>

              <p className="rounded-xl border border-border bg-surface px-3.5 py-3 text-[11px] leading-relaxed text-ink-muted">
                Este aplicativo <strong className="text-ink">não</strong> terá acesso às suas senhas,
                chaves privadas ou capacidade de movimentar seus fundos. Você pode revogar o acesso a
                qualquer momento nas configurações.
              </p>

              {user && (
                <div className="flex flex-col-reverse gap-2.5 sm:flex-row pt-1">
                  <button
                    type="button"
                    disabled={submitting}
                    onClick={() => handleDecision('deny')}
                    className="flex-1 rounded-xl border border-border bg-paper py-3 text-xs font-bold text-ink-muted hover:bg-surface hover:text-ink disabled:opacity-50"
                  >
                    Cancelar
                  </button>
                  <button
                    type="button"
                    disabled={submitting}
                    onClick={() => handleDecision('approve')}
                    className="flex-[1.4] inline-flex items-center justify-center gap-2 rounded-xl bg-bitcoin py-3 text-xs font-black text-white shadow-sm hover:bg-bitcoin-light disabled:opacity-50"
                  >
                    {submitting ? (
                      <>
                        <span className="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent" />
                        Autorizando…
                      </>
                    ) : (
                      <>
                        <i className="bi bi-check-circle-fill" />
                        Autorizar e continuar
                      </>
                    )}
                  </button>
                </div>
              )}
            </div>
          </div>

          <p className="mt-4 text-center text-[10px] text-ink-muted">
            Protegido por <span className="font-semibold text-ink">SatsPay Identity</span> · OAuth 2.0
            & OpenID Connect
          </p>
        </div>
      </main>
    </div>
  );
}

function PermissionItem({
  icon,
  title,
  body,
}: {
  icon: string;
  title: string;
  body: string;
}) {
  return (
    <li className="flex items-start gap-2.5 rounded-xl border border-border/80 bg-paper px-3 py-2.5">
      <div className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-lg bg-emerald-50 text-emerald-700">
        <i className={`bi ${icon} text-xs`} />
      </div>
      <div className="min-w-0">
        <div className="text-xs font-semibold text-ink">{title}</div>
        <p className="mt-0.5 text-[11px] leading-relaxed text-ink-muted">{body}</p>
      </div>
    </li>
  );
}

/** Reject SatsPay marketing assets used by mistake as the third-party app icon. */
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

/** Google s2 favicon as last-resort mark when the OAuth app has no logo_url. */
function websiteFaviconUrl(websiteUrl: string): string | undefined {
  try {
    const host = new URL(websiteUrl).hostname;
    if (!host) return undefined;
    return `https://www.google.com/s2/favicons?domain=${encodeURIComponent(host)}&sz=128`;
  } catch {
    return undefined;
  }
}

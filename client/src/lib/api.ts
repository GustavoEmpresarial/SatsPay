import { useAuthStore } from '../stores/auth.js';
import { useAdminStore } from '../stores/admin.js';
import { reportClientError, shouldReportApiStatus } from './reportError.js';

const BASE = '/v1';

export interface ApiOptions extends RequestInit {
  json?: unknown;
  skipAuth?: boolean;
  _retried?: boolean;
}

export class ApiError extends Error {
  status: number;
  code: string;
  details?: unknown;
  constructor(status: number, code: string, message: string, details?: unknown) {
    super(message);
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

/** True when the last refresh attempt was rejected as expired/revoked (not a network blip). */
let refreshAuthDenied = false;

function isAuthPublicPath(pathname: string): boolean {
  return (
    pathname === '/' ||
    pathname === '/welcome' ||
    pathname === '/login' ||
    pathname === '/register' ||
    pathname.startsWith('/admin/login') ||
    pathname.startsWith('/oauth/')
  );
}

/**
 * Clear session and send the user to login. Hard navigation so a dead session
 * cannot leave the UI stuck showing "sessão expirada" on a protected page.
 */
export function forceReauth(preferAdmin = false): void {
  useAuthStore.getState().logout();
  useAdminStore.getState().logout();
  if (typeof window === 'undefined') return;
  const path = window.location.pathname;
  if (isAuthPublicPath(path)) return;
  const dest = preferAdmin || path.startsWith('/admin') ? '/admin/login' : '/login';
  if (path !== dest) {
    window.location.replace(dest);
  }
}

export function wasRefreshAuthDenied(): boolean {
  return refreshAuthDenied;
}

let refreshInFlight: Promise<string | null> | null = null;

type PublicUserLike = Record<string, unknown> | null | undefined;

/** Map API user payloads onto the PublicUser field names the UI reads. */
function normalizeUser(user: PublicUserLike): PublicUserLike {
  if (!user || typeof user !== 'object') return user;
  const twoFactorEnabled =
    typeof user.twoFactorEnabled === 'boolean'
      ? user.twoFactorEnabled
      : typeof user.two_factor_enabled === 'boolean'
        ? user.two_factor_enabled
        : false;
  const merchantStatus =
    (typeof user.merchantStatus === 'string' && user.merchantStatus) ||
    (typeof user.merchant_status === 'string' && user.merchant_status) ||
    'NONE';
  const username =
    typeof user.username === 'string'
      ? user.username
      : typeof user.userName === 'string'
        ? user.userName
        : '';
  return {
    ...user,
    username,
    twoFactorEnabled,
    merchantStatus,
  };
}

/**
 * Normalize Rust api-http JSON → shapes the legacy web UI expects.
 * Paths already match (`/v1/auth/*`, `/v1/wallet`); only envelopes differ.
 * Exported for unit tests.
 */
export function adaptRustResponse(path: string, data: unknown): unknown {
  if (data == null || typeof data !== 'object') return data;

  // Login 2FA challenge: { kind: "code_sent", email } → { codeSent: true, message }
  if (path === '/auth/login' && 'kind' in data && (data as { kind: string }).kind === 'code_sent') {
    const email = (data as { email?: string }).email ?? '';
    return { codeSent: true as const, message: email ? `A verification code was sent to ${email}` : 'A verification code was sent to your email' };
  }

  // Register / login ok: { user, tokens: { accessToken } } → flat session (refresh is HttpOnly cookie only)
  // Also accept kind: "ok" envelopes from the Rust API.
  if (path === '/auth/register' || path === '/auth/login') {
    const body = data as {
      user?: PublicUserLike;
      tokens?: { accessToken?: string };
      accessToken?: string;
      kind?: string;
    };
    if (body.kind === 'code_sent') {
      // already handled above; keep fallthrough safe
    } else if (body.tokens?.accessToken || body.accessToken) {
      return {
        user: normalizeUser(body.user),
        accessToken: body.tokens?.accessToken ?? body.accessToken,
      };
    }
  }

  // /auth/me → ensure username/2FA field names are normalized
  if (path === '/auth/me' && data && typeof data === 'object' && 'user' in data) {
    const body = data as { user: PublicUserLike };
    return { user: normalizeUser(body.user) };
  }

  // Refresh ok: { accessToken } (cookie rotates HttpOnly refresh; never returned in JSON)
  if (path === '/auth/refresh') {
    const body = data as {
      accessToken?: string;
      access_token?: string;
      tokens?: { accessToken?: string; access_token?: string };
    };
    return {
      accessToken: body.accessToken ?? body.access_token ?? body.tokens?.accessToken ?? body.tokens?.access_token,
    };
  }

  // Wallet list: bare array → { wallets: [...] } (query ?kind= is ignored by Rust; always PERSONAL)
  if ((path === '/wallet' || path.startsWith('/wallet?')) && Array.isArray(data)) {
    return { wallets: data };
  }

  return data;
}

export function adaptRustError(data: unknown, statusText: string): { code: string; message: string; details?: unknown } {
  const err = (data as { error?: unknown } | null)?.error;
  if (typeof err === 'string') {
    // Legacy string errors (e.g. raw cooldown timestamp dumps).
    if (/next claim available at/i.test(err)) {
      const match = err.match(/next claim available at\s+(.+)$/i);
      return {
        code: 'FAUCET_COOLDOWN',
        message: 'Aguarde o cooldown de 11 horas do faucet.',
        details: match?.[1] ? { nextClaimAt: match[1].trim() } : undefined,
      };
    }
    return { code: 'ERROR', message: err };
  }
  if (err && typeof err === 'object' && 'message' in err) {
    const e = err as {
      code?: string;
      message: string;
      details?: unknown;
      nextClaimAt?: string;
      next_claim_at?: string;
    };
    const nextClaimAt = e.nextClaimAt ?? e.next_claim_at;
    return {
      code: e.code ?? 'UNKNOWN',
      message: e.message,
      details: e.details ?? (nextClaimAt ? { nextClaimAt } : undefined),
    };
  }
  return { code: 'UNKNOWN', message: statusText };
}

/** True when a raw fetch body looks like JSON we must tag with Content-Type. */
export function looksLikeJsonBody(body: string): boolean {
  const trimmed = body.trimStart();
  return trimmed.startsWith('{') || trimmed.startsWith('[');
}

/** Mint a new access token from the HttpOnly refresh cookie (never from localStorage). */
export async function refreshAccessToken(): Promise<string | null> {
  if (refreshInFlight) return refreshInFlight;

  refreshInFlight = (async () => {
    try {
      const res = await fetch(`${BASE}/auth/refresh`, {
        method: 'POST',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        // Empty body — server reads refresh_token cookie only.
        body: '{}',
      });

      // If the server explicitly rejected the refresh token (revoked / expired / invalid / 401 / 403)
      if (res.status === 401 || res.status === 403) {
        refreshAuthDenied = true;
        forceReauth();
        return null;
      }

      refreshAuthDenied = false;

      // If the server is restarting (502 Bad Gateway, 503, 504), don't logout, just return null
      if (!res.ok) return null;

      const raw = (await res.json().catch(() => ({}))) as unknown;
      const data = adaptRustResponse('/auth/refresh', raw) as { accessToken?: string };
      if (!data.accessToken) return null;

      // Memory only — Set-Cookie rotates the HttpOnly refresh token.
      useAuthStore.setState({ accessToken: data.accessToken });
      useAdminStore.setState({ accessToken: data.accessToken });
      return data.accessToken;
    } catch {
      // Network error (e.g. backend down or restarting during deploy) — keep session
      refreshAuthDenied = false;
      return null;
    } finally {
      refreshInFlight = null;
    }
  })();

  return refreshInFlight;
}

/**
 * After reload: user/admin may be in localStorage but access JWT is gone.
 * Restore access via HttpOnly refresh cookie. Returns true if we have a usable access token.
 */
export async function bootstrapSession(): Promise<boolean> {
  const auth = useAuthStore.getState();
  const admin = useAdminStore.getState();
  if (auth.accessToken || admin.accessToken) return true;

  // No persisted identity and no in-memory token — nothing to restore.
  if (!auth.user && !admin.admin) return false;

  const token = await refreshAccessToken();
  if (!token) {
    if (wasRefreshAuthDenied()) {
      // forceReauth already sent the browser to /login
      return false;
    }
    // Transient refresh failure with a stale persisted user — clear identity so RequireAuth
    // redirects to login (soft Navigate; hard redirect only for definitive auth deny).
    useAuthStore.getState().logout();
    useAdminStore.getState().logout();
    return false;
  }
  return true;
}

export async function api<T = unknown>(path: string, opts: ApiOptions = {}): Promise<T> {
  const { json, skipAuth, headers, _retried, ...rest } = opts;
  const auth = useAuthStore.getState();
  const admin = useAdminStore.getState();
  const isAdminPath = path.startsWith('/admin') && !path.startsWith('/admin/login');
  const token = isAdminPath
    ? admin.accessToken || auth.accessToken
    : auth.accessToken || admin.accessToken;

  const body = json !== undefined ? JSON.stringify(json) : rest.body;
  const finalHeaders: Record<string, string> = {
    ...(headers as Record<string, string> | undefined),
  };
  // Axum Json extractors return 415 without this — never send a JSON body bare.
  if (json !== undefined || (typeof body === 'string' && body.length > 0 && looksLikeJsonBody(body))) {
    if (!finalHeaders['Content-Type'] && !finalHeaders['content-type']) {
      finalHeaders['Content-Type'] = 'application/json';
    }
  }
  if (!skipAuth && token) {
    finalHeaders.Authorization = `Bearer ${token}`;
  }

  let res: Response;
  try {
    res = await fetch(`${BASE}${path}`, {
      ...rest,
      headers: finalHeaders,
      credentials: 'include',
      body,
    });
  } catch {
    reportClientError({
      kind: 'network',
      message: `NETWORK ${path}`,
      endpoint: path,
      statusCode: 0,
    });
    throw new ApiError(0, 'NETWORK', 'API unreachable — is the backend proxy up?');
  }

  if (res.status === 204) return undefined as T;

  const data = (await res.json().catch(() => ({}))) as unknown;

  // Auto-refresh on 401 (once). Expired session → login; deploy blip → keep identity if refresh is only transient.
  if (res.status === 401 && !skipAuth) {
    if (!_retried) {
      const newToken = await refreshAccessToken();
      if (newToken) {
        return api<T>(path, { ...opts, _retried: true });
      }
      if (wasRefreshAuthDenied()) {
        // forceReauth already navigated to /login
        throw new ApiError(401, 'UNAUTHORIZED', 'Sessão expirada. Faça login novamente.');
      }
      // Refresh failed transiently (5xx/network) — do not wipe the session.
      throw new ApiError(401, 'UNAUTHORIZED', 'Sessão temporariamente indisponível. Tente de novo.');
    }
    // Retried and still 401 — session is dead.
    forceReauth(isAdminPath);
    throw new ApiError(401, 'UNAUTHORIZED', 'Sessão expirada. Faça login novamente.');
  }

  if (!res.ok) {
    const err = adaptRustError(data, res.statusText);
    const message =
      (err.message === 'Internal Server Error' || err.message === 'Bad Gateway' || !err.message)
        ? `API error ${res.status} — backend may be down or misconfigured`
        : err.message;
    if (shouldReportApiStatus(res.status, path, message)) {
      reportClientError({
        kind: path.includes('/auth/') ? 'auth' : path.includes('/oauth/') ? 'oauth' : 'api',
        message: `${res.status} ${path}: ${message}`,
        endpoint: path,
        statusCode: res.status,
        context: { code: err.code },
        level: res.status >= 500 ? 'CRITICAL' : 'ERROR',
      });
    }
    throw new ApiError(res.status, err.code, message, err.details);
  }
  return adaptRustResponse(path, data) as T;
}

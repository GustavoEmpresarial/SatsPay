import { useAuthStore } from '../stores/auth.js';

export const CLIENT_ERROR_KINDS = [
  'js',
  'promise',
  'react',
  'api',
  'network',
  'auth',
  'oauth',
  'captcha',
  'resource',
  'csp',
  'console',
  'query',
] as const;
export type ClientErrorKind = (typeof CLIENT_ERROR_KINDS)[number];

export interface Breadcrumb {
  category: 'navigation' | 'ui.click' | 'http' | 'console' | 'auth' | 'oauth';
  message: string;
  timestamp: string;
  data?: Record<string, unknown>;
}

const MAX_BREADCRUMBS = 40;
const breadcrumbs: Breadcrumb[] = [];

export function addBreadcrumb(
  category: Breadcrumb['category'],
  message: string,
  data?: Record<string, unknown>,
): void {
  const item: Breadcrumb = {
    category,
    message: message.slice(0, 240),
    timestamp: new Date().toISOString(),
    data,
  };
  breadcrumbs.push(item);
  if (breadcrumbs.length > MAX_BREADCRUMBS) {
    breadcrumbs.shift();
  }
}

export function getBreadcrumbs(): Breadcrumb[] {
  return [...breadcrumbs];
}

/** Paths where 4xx failures are product-critical and should be collected. */
const CRITICAL_API_PATH_PREFIXES = [
  '/auth/',
  '/oauth/',
  '/wallet',
  '/swap',
  '/faucet',
  '/public/',
  '/admin/',
  '/telemetry/',
];

export const HTTP_SERVER_ERROR_MIN = 500;

export function isCriticalApiPath(path: string): boolean {
  const p = path.split('?')[0] || path;
  return CRITICAL_API_PATH_PREFIXES.some((prefix) => p === prefix || p.startsWith(prefix));
}

/**
 * Massive collection policy:
 * - always network (0) + 5xx
 * - 408/409/415/429 always
 * - other 4xx on critical product paths (auth/oauth/wallet/…)
 */
export function shouldReportApiStatus(status: number, path = '', message = ''): boolean {
  if (isExpectedApiNoise(status, path, message)) return false;
  if (status === 0 || status >= HTTP_SERVER_ERROR_MIN) return true;
  if (status === 408 || status === 409 || status === 415 || status === 429) return true;
  if (status >= 400 && status < 500 && isCriticalApiPath(path)) return true;
  return false;
}

export function isExternalNoise(stackOrUrl?: string, message?: string): boolean {
  const stack = stackOrUrl || '';
  const msg = (message || '').toLowerCase();
  // Only the failing script's URL may identify an external source. Matching a
  // domain anywhere in an error message can hide an application error whose
  // text happens to mention that domain.
  const candidate = stack.match(/(?:https?:\/\/|chrome-extension:\/\/|moz-extension:\/\/|safari-extension:\/\/)[^\s)]+/i)?.[0];
  let external = false;
  if (candidate) {
    try {
      const url = new URL(candidate);
      const host = url.hostname.toLowerCase();
      const registrableHost = host.split('.').slice(-2).join('.');
      external = ['cloudflareinsights.com', 'googletagmanager.com', 'google-analytics.com', 'facebook.net', 'doubleclick.net']
        .includes(registrableHost)
        || ['chrome-extension:', 'moz-extension:', 'safari-extension:'].includes(url.protocol)
        || (host === 'cdn.jsdelivr.net' && url.pathname.startsWith('/gh/atomiclabs/cryptocurrency-icons'))
        || (host === 'raw.githubusercontent.com' && url.pathname.startsWith('/solana-labs/token-list'))
        || (host === 'surfe.pro' && url.pathname.startsWith('/track'));
    } catch {
      external = false;
    }
  }
  return (
    external ||
    msg.includes('resizeobserver loop') ||
    msg.includes('script error.') ||
    msg.includes('unhandledrejection: abort') ||
    // Expected product / auth noise — not actionable bugs
    msg.includes('aguarde o cooldown') ||
    msg.includes('next claim available') ||
    msg.includes('invalid email or password') ||
    msg.includes('too many requests') ||
    msg.includes('captcha verification failed') ||
    msg.includes('sessão expirada') ||
    msg.includes('platform inventory') ||
    msg.includes('insufficient for this operation') ||
    msg.includes('removechild') ||
    msg.includes('minified react error #311')
  );
}

/** Expected API failures that should not flood admin telemetry. */
export function isExpectedApiNoise(status: number, path: string, message?: string): boolean {
  const p = (path.split('?')[0] || path).toLowerCase();
  const msg = (message || '').toLowerCase();
  if (p.includes('/faucet/claim') && (status === 429 || status === 400)) return true;
  if (p.includes('/auth/login') && (status === 400 || status === 401 || status === 429)) return true;
  // Session probe during restart / cold proxy blip
  if (p.includes('/auth/me') && (status === 401 || status === 502 || status === 503 || status === 0)) return true;
  if (p.includes('/swap/quote') && (status === 400 || status === 404) && msg.includes('no routes')) return true;
  if (msg.includes('no routes available')) return true;
  if (msg.includes('aguarde o cooldown') || msg.includes('next claim available')) return true;
  if (msg.includes('invalid email or password') || msg.includes('too many requests')) return true;
  if (msg.includes('platform inventory') && msg.includes('insufficient')) return true;
  if (msg.includes('inventário') && msg.includes('insuficiente')) return true;
  return false;
}

export function isClientErrorKind(value: string): value is ClientErrorKind {
  return CLIENT_ERROR_KINDS.some((kind) => kind === value);
}

export interface ClientErrorReport {
  kind: ClientErrorKind;
  message: string;
  stack?: string;
  endpoint?: string;
  statusCode?: number;
  context?: Record<string, unknown>;
  level?: 'ERROR' | 'WARN' | 'CRITICAL';
}

type QueuedReport = ClientErrorReport & { queuedAt: string; suppressedCount: number };

const reportedTimestamps = new Map<string, number>();
const suppressedCounts = new Map<string, number>();
const THROTTLE_MS = 12_000;
const MAX_QUEUE = 40;
const FLUSH_MS = 2_500;

const queue: QueuedReport[] = [];
let flushTimer: ReturnType<typeof setTimeout> | null = null;
let collectorsInstalled = false;

function reportKey(report: ClientErrorReport): string {
  const firstLine = (report.message || '').split('\n')[0] ?? '';
  const msgKey = firstLine.slice(0, 100);
  return `${report.kind}:${report.endpoint ?? ''}:${report.statusCode ?? ''}:${msgKey}`;
}

function buildTransportPayload(report: ClientErrorReport & { suppressedCount?: number }) {
  return {
    message: report.message.slice(0, 2000),
    stack: report.stack?.slice(0, 8000),
    url: report.endpoint ?? (typeof window !== 'undefined' ? window.location.href : undefined),
    user_agent: typeof navigator !== 'undefined' ? navigator.userAgent : undefined,
    kind: report.kind,
    status_code: report.statusCode,
    level: report.level ?? (report.statusCode && report.statusCode >= 500 ? 'CRITICAL' : 'ERROR'),
    context: {
      screen: typeof window !== 'undefined' ? `${window.innerWidth}x${window.innerHeight}` : undefined,
      language: typeof navigator !== 'undefined' ? navigator.language : undefined,
      online: typeof navigator !== 'undefined' ? navigator.onLine : undefined,
      pathname: typeof window !== 'undefined' ? window.location.pathname : undefined,
      search: typeof window !== 'undefined' ? window.location.search.slice(0, 200) : undefined,
      visibility: typeof document !== 'undefined' ? document.visibilityState : undefined,
      breadcrumbs: [...breadcrumbs].slice(-25),
      suppressedCount: report.suppressedCount ?? 0,
      ...report.context,
    },
  };
}

function authHeaders(): Record<string, string> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  try {
    const token = useAuthStore.getState().accessToken;
    if (token) headers.Authorization = `Bearer ${token}`;
  } catch {
    // store may be unavailable during early boot
  }
  return headers;
}

function postBatch(errors: ReturnType<typeof buildTransportPayload>[]): void {
  if (typeof window === 'undefined' || errors.length === 0) return;
  const body = JSON.stringify({ errors });
  const url = '/v1/telemetry/client-errors';
  try {
    if (typeof navigator !== 'undefined' && typeof navigator.sendBeacon === 'function' && errors.length <= 5) {
      const blob = new Blob([body], { type: 'application/json' });
      if (navigator.sendBeacon(url, blob)) return;
    }
  } catch {
    // fall through to fetch
  }
  void fetch(url, {
    method: 'POST',
    credentials: 'include',
    headers: authHeaders(),
    body,
    keepalive: true,
  }).catch(() => undefined);
}

function scheduleFlush(): void {
  if (flushTimer != null) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    flushErrorQueue();
  }, FLUSH_MS);
}

export function flushErrorQueue(): void {
  if (queue.length === 0) return;
  const batch = queue.splice(0, MAX_QUEUE);
  postBatch(batch.map((r) => buildTransportPayload(r)));
}

export function reportClientError(report: ClientErrorReport): void {
  if (typeof window === 'undefined') return;
  if (isExternalNoise(report.stack, report.message)) return;
  if (isExpectedApiNoise(report.statusCode ?? 0, report.endpoint ?? '', report.message)) return;
  if (report.endpoint?.includes('/telemetry/client-error')) return;

  const key = reportKey(report);
  const now = Date.now();
  const lastReported = reportedTimestamps.get(key) ?? 0;
  if (now - lastReported < THROTTLE_MS) {
    suppressedCounts.set(key, (suppressedCounts.get(key) ?? 0) + 1);
    return;
  }
  const suppressed = suppressedCounts.get(key) ?? 0;
  suppressedCounts.set(key, 0);
  reportedTimestamps.set(key, now);

  addBreadcrumb('http', `[${report.kind}] ${report.message.slice(0, 120)}`, {
    statusCode: report.statusCode,
    endpoint: report.endpoint,
  });

  queue.push({
    ...report,
    queuedAt: new Date().toISOString(),
    suppressedCount: suppressed,
  });
  if (queue.length >= 8) {
    flushErrorQueue();
  } else {
    scheduleFlush();
  }
}

/** Convenience for auth/oauth/captcha UI failures. */
export function reportAuthFailure(
  kind: 'auth' | 'oauth' | 'captcha',
  message: string,
  context?: Record<string, unknown>,
): void {
  reportClientError({
    kind,
    message,
    endpoint: typeof window !== 'undefined' ? window.location.pathname : undefined,
    context,
  });
}

function onWindowError(event: ErrorEvent): void {
  const target = event.target as HTMLElement | null | undefined;
  if (target && (target as HTMLElement).tagName) {
    const tag = target.tagName.toLowerCase();
    if (tag === 'script' || tag === 'link' || tag === 'img' || tag === 'source' || tag === 'video') {
      const src =
        (target as HTMLImageElement).src ||
        (target as HTMLScriptElement).src ||
        (target as HTMLLinkElement).href ||
        '';
      if (isExternalNoise(src)) return;
      reportClientError({
        kind: 'resource',
        message: `Failed to load <${tag}> ${src.slice(0, 180)}`,
        endpoint: src.slice(0, 300),
        context: { tag },
        level: tag === 'script' ? 'CRITICAL' : 'WARN',
      });
      return;
    }
  }

  const stack = event.error?.stack || `at ${event.filename}:${event.lineno}:${event.colno}`;
  reportClientError({
    kind: 'js',
    message: event.message || 'Unknown client error',
    stack,
  });
}

function onUnhandledRejection(event: PromiseRejectionEvent): void {
  const reason = event.reason;
  const message =
    typeof reason === 'string' ? reason : reason?.message || 'Unhandled Promise Rejection';
  const stack =
    typeof reason === 'object' && reason && 'stack' in reason ? String(reason.stack) : undefined;
  reportClientError({
    kind: 'promise',
    message: String(message).slice(0, 500),
    stack,
  });
}

function onSecurityPolicyViolation(event: SecurityPolicyViolationEvent): void {
  reportClientError({
    kind: 'csp',
    message: `CSP ${event.violatedDirective}: ${event.blockedURI || event.documentURI}`,
    context: {
      effectiveDirective: event.effectiveDirective,
      disposition: event.disposition,
      blockedURI: event.blockedURI,
      sourceFile: event.sourceFile,
      lineNumber: event.lineNumber,
    },
    level: 'WARN',
  });
}

/**
 * Install global collectors once. Safe to call repeatedly.
 * Captures: window errors, promise rejections, CSP, resource loads, console.error.
 */
export function installErrorCollectors(): void {
  if (typeof window === 'undefined' || collectorsInstalled) return;
  collectorsInstalled = true;

  window.addEventListener('error', onWindowError, true);
  window.addEventListener('unhandledrejection', onUnhandledRejection);
  window.addEventListener('securitypolicyviolation', onSecurityPolicyViolation);
  window.addEventListener('pagehide', () => flushErrorQueue());
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') flushErrorQueue();
  });

  document.addEventListener(
    'click',
    (event) => {
      try {
        const target = event.target as HTMLElement | null;
        if (!target) return;
        const tag = target.tagName.toLowerCase();
        const role = target.getAttribute('role') || '';
        const id = target.id ? `#${target.id}` : '';
        const text = (target.innerText || target.getAttribute('aria-label') || '').slice(0, 35).trim();
        const info = [tag + id, role, text ? `"${text}"` : ''].filter(Boolean).join(' ');
        addBreadcrumb('ui.click', `Clicked ${info}`);
      } catch {
        // ignore
      }
    },
    { capture: true, passive: true },
  );

  // Deliberate console wrap: console.error output becomes a breadcrumb + report.
  // eslint-disable-next-line no-console
  const originalConsoleError = console.error;
  // eslint-disable-next-line no-console
  console.error = (...args: unknown[]) => {
    try {
      const msg = args
        .map((a) => {
          if (typeof a === 'string') return a;
          if (a instanceof Error) return a.message;
          try {
            return JSON.stringify(a);
          } catch {
            return String(a);
          }
        })
        .join(' ')
        .slice(0, 300);
      addBreadcrumb('console', msg);
      if (!isExternalNoise(undefined, msg)) {
        reportClientError({
          kind: 'console',
          message: msg,
          level: 'WARN',
        });
      }
    } catch {
      // ignore
    }
    originalConsoleError.apply(console, args);
  };
}

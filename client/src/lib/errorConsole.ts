/** Classificação + redação estilo console de operações (TubeCoin). Admin UI only. */

export type ErrorSeverity = 'FATAL' | 'ERROR' | 'WARNING' | 'INFO';
export type ErrorImpact = 'CRITICAL' | 'HIGH' | 'MEDIUM' | 'LOW';
export type ErrorCategory =
  | 'AUTH'
  | 'BUSINESS'
  | 'CLIENT'
  | 'INFRA'
  | 'LEDGER'
  | 'NETWORK'
  | 'UNKNOWN';
export type ErrorLifecycle = 'NEW' | 'OPEN' | 'INVESTIGATING' | 'RESOLVED' | 'IGNORED';

export interface ClassifiedError {
  severity: ErrorSeverity;
  impact: ErrorImpact;
  category: ErrorCategory;
  lifecycle: ErrorLifecycle;
  code: string;
  origin: string;
  sample: string;
  path: string;
  errorIdShort: string;
}

const SECRET_PATTERNS: Array<{ re: RegExp; replace: string | ((...args: string[]) => string) }> = [
  { re: /\bBearer\s+[A-Za-z0-9\-._~+/]+=*/gi, replace: 'Bearer [REDACTED]' },
  {
    re: /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/g,
    replace: '[REDACTED_JWT]',
  },
  {
    re: /\b(?:sk|pk|ak)_[A-Za-z0-9_]{8,}/gi,
    replace: '[REDACTED_KEY]',
  },
  {
    re: /\b(?:ENCRYPTION_KEY|JWT_ACCESS_SECRET|HOT_MNEMONIC(?:_ENC)?|SMTP_PASSWORD|TURNSTILE_SECRET)\s*[=:]\s*\S+/gi,
    replace: '[REDACTED_ENV]',
  },
  { re: /\b[0-9a-f]{64}\b/gi, replace: '[REDACTED_HEX]' },
  {
    re: /\b(?:password|passwd|secret|token|api[_-]?key)\s*[=:]\s*\S+/gi,
    replace: '[REDACTED_SECRET]',
  },
];

export function redactSecrets(input: string | undefined | null): string {
  if (!input) return '';
  let out = input;
  for (const { re, replace } of SECRET_PATTERNS) {
    out = out.replace(re, replace as string);
  }
  return out;
}

function deriveCode(message: string, fingerprint: string): string {
  const m = message.trim();
  const known: Array<[RegExp, string]> = [
    [/not logged in/i, 'AUTH_NOT_LOGGED_IN'],
    [/wrong (address|email|password)|invalid credentials/i, 'AUTH_INVALID_CREDENTIALS'],
    [/already registered|address already/i, 'AUTH_ADDRESS_TAKEN'],
    [/not enough zems.*ticket/i, 'INSUFFICIENT_ZEMS'],
    [/not enough zems.*donat/i, 'INSUFFICIENT_ZEMS'],
    [/not enough zems/i, 'INSUFFICIENT_ZEMS'],
    [/tile already done/i, 'TILE_ALREADY_DONE'],
    [/no girl is ready|girl.?not.?ready/i, 'GIRL_NOT_READY'],
    [/provider is disconnected/i, 'CLIENT_JS_ERROR'],
    [/csrf/i, 'CSRF_REJECTED'],
    [/rate.?limit/i, 'RATE_LIMITED'],
    [/insufficient (balance|funds)/i, 'INSUFFICIENT_BALANCE'],
  ];
  for (const [re, code] of known) {
    if (re.test(m)) return code;
  }
  const slug = m
    .replace(/[^a-zA-Z0-9]+/g, '_')
    .replace(/^_|_$/g, '')
    .slice(0, 28)
    .toUpperCase();
  if (slug.length >= 4) return slug;
  return `ERR_${fingerprint.slice(0, 8).toUpperCase()}`;
}

function mapSeverity(level: string): ErrorSeverity {
  const l = level.toUpperCase();
  if (l === 'FATAL' || l === 'CRITICAL') return 'FATAL';
  if (l === 'ERROR') return 'ERROR';
  if (l === 'WARN' || l === 'WARNING') return 'WARNING';
  return 'INFO';
}

function mapImpact(err: {
  level: string;
  status_code?: number;
  occurrences_count: number;
  service: string;
}): ErrorImpact {
  const sev = mapSeverity(err.level);
  if (sev === 'FATAL') return 'CRITICAL';
  if (err.status_code && err.status_code >= 500) return 'HIGH';
  if (err.occurrences_count >= 50) return 'HIGH';
  if (err.occurrences_count >= 10 || (err.status_code && err.status_code >= 400)) return 'MEDIUM';
  if (err.service.includes('client')) return 'LOW';
  return 'MEDIUM';
}

function mapCategory(err: {
  service: string;
  message: string;
  status_code?: number;
  endpoint?: string;
}): ErrorCategory {
  const msg = err.message.toLowerCase();
  const svc = err.service.toLowerCase();
  if (
    msg.includes('not logged') ||
    msg.includes('credentials') ||
    msg.includes('unauthorized') ||
    msg.includes('csrf') ||
    msg.includes('already registered') ||
    err.status_code === 401 ||
    err.status_code === 403
  ) {
    return 'AUTH';
  }
  if (
    msg.includes('insufficient') ||
    msg.includes('not enough') ||
    msg.includes('already done') ||
    msg.includes('not ready') ||
    (err.status_code !== undefined && err.status_code >= 400 && err.status_code < 500)
  ) {
    return 'BUSINESS';
  }
  if (svc.includes('client') || msg.includes('provider is disconnected') || msg.includes('failed to load')) {
    return 'CLIENT';
  }
  if (msg.includes('ledger') || msg.includes('double.?entry') || msg.includes('wallet')) {
    return 'LEDGER';
  }
  if (err.status_code && err.status_code >= 500) return 'INFRA';
  if (msg.includes('timeout') || msg.includes('econn') || msg.includes('network')) return 'NETWORK';
  return 'UNKNOWN';
}

function mapLifecycle(status: string, firstSeen: string, lastSeen: string): ErrorLifecycle {
  const s = status.toUpperCase();
  if (s === 'RESOLVED') return 'RESOLVED';
  if (s === 'IGNORED') return 'IGNORED';
  if (s === 'INVESTIGATING') return 'INVESTIGATING';
  // NEW if first seen within ~36h of last seen and still OPEN
  const first = Date.parse(firstSeen);
  const last = Date.parse(lastSeen);
  if (Number.isFinite(first) && Number.isFinite(last) && last - first < 36 * 3600_000) {
    return 'NEW';
  }
  return 'OPEN';
}

function mapOrigin(service: string, statusCode?: number): string {
  const s = service.toLowerCase();
  if (s.includes('client') && statusCode) return 'client_api';
  if (s.includes('client')) return 'client';
  if (s.includes('worker')) return 'worker';
  return 'api';
}

export function classifyError(err: {
  id: string;
  fingerprint: string;
  service: string;
  level: string;
  message: string;
  endpoint?: string;
  status_code?: number;
  occurrences_count: number;
  status: string;
  first_seen_at: string;
  last_seen_at: string;
}): ClassifiedError {
  const sample = redactSecrets(err.message);
  return {
    severity: mapSeverity(err.level),
    impact: mapImpact(err),
    category: mapCategory(err),
    lifecycle: mapLifecycle(err.status, err.first_seen_at, err.last_seen_at),
    code: deriveCode(err.message, err.fingerprint),
    origin: mapOrigin(err.service, err.status_code),
    sample: sample.length > 80 ? `${sample.slice(0, 77)}…` : sample,
    path: err.endpoint ? redactSecrets(err.endpoint) : '—',
    errorIdShort: err.id.length > 12 ? `${err.id.slice(0, 6)}…${err.id.slice(-4)}` : err.id,
  };
}

export function isExpectedAuthNoise(err: { message: string; status_code?: number; endpoint?: string }): boolean {
  const m = err.message.toLowerCase();
  const p = (err.endpoint || '').toLowerCase();
  if (m.includes('not logged in')) return true;
  if (err.status_code === 401 && (m.includes('unauthorized') || m.includes('not logged') || p.includes('/auth/me'))) {
    return true;
  }
  // Deploy / proxy blips
  if (p.includes('/auth/me') && (err.status_code === 502 || err.status_code === 503 || err.status_code === 0)) {
    return true;
  }
  if (m.includes('backend may be down') && p.includes('/auth/me')) return true;
  if (m.includes('network') && p.includes('/auth/me')) return true;
  if (m.includes('no routes available')) return true;
  if (m.includes('platform inventory') && m.includes('insufficient')) return true;
  return false;
}

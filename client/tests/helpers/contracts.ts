/**
 * Consumer-driven contract helpers — frontend ↔ API JSON shapes (#6, #54).
 * Keep in sync with crates/api-http auth UserResponse / TokensResponse.
 */

export interface ContractIssue {
  path: string;
  message: string;
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function assertPublicUser(raw: unknown, path = 'user'): ContractIssue[] {
  const issues: ContractIssue[] = [];
  if (!isObject(raw)) {
    issues.push({ path, message: 'expected object' });
    return issues;
  }
  for (const key of ['id', 'email', 'username', 'createdAt'] as const) {
    if (typeof raw[key] !== 'string' || !(raw[key] as string).length) {
      issues.push({ path: `${path}.${key}`, message: 'expected non-empty string' });
    }
  }
  if (typeof raw.twoFactorEnabled !== 'boolean') {
    issues.push({ path: `${path}.twoFactorEnabled`, message: 'expected boolean' });
  }
  if (typeof raw.merchantStatus !== 'string') {
    issues.push({ path: `${path}.merchantStatus`, message: 'expected string' });
  }
  return issues;
}

export function assertAuthLoginResponse(raw: unknown): ContractIssue[] {
  const issues: ContractIssue[] = [];
  if (!isObject(raw)) return [{ path: '$', message: 'expected object' }];
  issues.push(...assertPublicUser(raw.user));
  if (typeof raw.accessToken !== 'string' || !raw.accessToken.length) {
    issues.push({ path: 'accessToken', message: 'expected non-empty string' });
  }
  // Refresh must NOT appear in JSON (HttpOnly cookie only) — regression guard
  if ('refreshToken' in raw && raw.refreshToken != null && raw.refreshToken !== '') {
    issues.push({ path: 'refreshToken', message: 'must not be returned in JSON body' });
  }
  return issues;
}

export function assertSwapPricesResponse(raw: unknown): ContractIssue[] {
  const issues: ContractIssue[] = [];
  if (!isObject(raw)) return [{ path: '$', message: 'expected object' }];
  if (typeof raw.priceDecimals !== 'number') {
    issues.push({ path: 'priceDecimals', message: 'expected number' });
  }
  if (!isObject(raw.prices)) {
    issues.push({ path: 'prices', message: 'expected object map' });
  } else {
    for (const [coin, val] of Object.entries(raw.prices)) {
      if (typeof val !== 'string' && typeof val !== 'number') {
        issues.push({ path: `prices.${coin}`, message: 'expected string|number' });
      }
    }
  }
  return issues;
}

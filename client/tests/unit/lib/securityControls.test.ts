import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const nginx = readFileSync(path.join(root, 'nginx.conf'), 'utf8');
const bridge = readFileSync(path.join(root, 'src/pages/OAuthPopupBridgePage.tsx'), 'utf8');
const authorize = readFileSync(path.join(root, 'src/pages/OAuthAuthorizePage.tsx'), 'utf8');
const returnTo = readFileSync(path.join(root, 'src/lib/returnTo.ts'), 'utf8');

/**
 * Security regression suite — maps to common React / browser OWASP controls:
 * CSP, clickjacking, MIME sniffing, OAuth postMessage targeting, open redirects.
 */
describe('client security headers (nginx)', () => {
  it('ships Content-Security-Policy that blocks default script injection', () => {
    expect(nginx).toContain('Content-Security-Policy');
    expect(nginx).toContain("default-src 'self'");
    expect(nginx).toContain("object-src 'none'");
    expect(nginx).toContain("frame-ancestors 'none'");
    expect(nginx).toContain('https://challenges.cloudflare.com'); // Turnstile
  });

  it('sets clickjacking + MIME + referrer baselines', () => {
    expect(nginx).toContain('X-Frame-Options "DENY"');
    expect(nginx).toContain('X-Content-Type-Options "nosniff"');
    expect(nginx).toContain('Referrer-Policy "strict-origin-when-cross-origin"');
  });

  it('keeps COOP compatible with OAuth popups', () => {
    expect(nginx).toContain('same-origin-allow-popups');
  });

  it('does not allow script-src unsafe-eval', () => {
    expect(nginx).not.toMatch(/script-src[^;]*unsafe-eval/);
  });
});

describe('OAuth bridge postMessage targeting', () => {
  it('targets merchant redirect origin instead of wildcard *', () => {
    expect(bridge).toContain('targetOrigin');
    expect(bridge).toContain('new URL(payload.redirectUrl).origin');
    expect(bridge).not.toMatch(/postMessage\([^)]+,\s*['"]\*['"]\s*\)/);
  });
});

describe('authorize page XSS hygiene', () => {
  it('does not use dangerouslySetInnerHTML', () => {
    expect(authorize).not.toContain('dangerouslySetInnerHTML');
  });

  it('renders app logos via img object-contain (no raw HTML inject)', () => {
    expect(authorize).toContain('object-contain');
    expect(authorize).not.toMatch(/innerHTML\s*=/);
  });
});

describe('open-redirect guard remains source-of-truth', () => {
  it('blocks protocol-relative and absolute URLs in returnTo', () => {
    expect(returnTo).toContain("startsWith('//')");
    expect(returnTo).toContain("includes('://')");
  });
});

describe('session secrets not in localStorage', () => {
  const authStore = readFileSync(
    path.join(root, 'src/stores/auth.ts'),
    'utf8',
  );
  it('auth persist version 2 drops tokens from partialize', () => {
    expect(authStore).toContain('version: 2');
    expect(authStore).toMatch(/partialize:[\s\S]*user: state\.user/);
    expect(authStore).not.toMatch(/partialize:[\s\S]*accessToken: state\.accessToken/);
  });
});

describe('refresh cookie __Host- (api-http source)', () => {
  const authRs = readFileSync(
    path.join(root, '../crates/api-http/src/auth.rs'),
    'utf8',
  );
  it('uses __Host-refresh_token when Secure is on', () => {
    expect(authRs).toContain('__Host-refresh_token');
    expect(authRs).toContain('TokensResponse');
    // Struct body must be access-only (refresh is cookie-only).
    const struct = authRs.match(/struct TokensResponse \{[^}]+\}/);
    expect(struct?.[0]).toBeTruthy();
    expect(struct![0]).toContain('access_token');
    expect(struct![0]).not.toContain('refresh_token');
  });
});

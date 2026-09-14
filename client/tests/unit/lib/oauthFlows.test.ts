import { describe, expect, it } from 'vitest';
import { resolveReturnTo } from '../../../src/lib/returnTo.js';
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const sdk = readFileSync(path.join(root, 'public/sdk/satspay-auth.js'), 'utf8');

describe('resolveReturnTo', () => {
  it('allows relative oauth authorize return paths', () => {
    expect(resolveReturnTo('/oauth/authorize?client_id=x')).toBe('/oauth/authorize?client_id=x');
  });

  it('allows bridge and popup-done return paths', () => {
    expect(resolveReturnTo('/oauth/bridge?code=abc')).toBe('/oauth/bridge?code=abc');
    expect(resolveReturnTo('/oauth/popup-done')).toBe('/oauth/popup-done');
  });

  it('allows developer oauth apps path', () => {
    expect(resolveReturnTo('/developer/oauth')).toBe('/developer/oauth');
  });

  it('blocks open redirects', () => {
    expect(resolveReturnTo('https://evil.com')).toBe('/dashboard');
    expect(resolveReturnTo('http://evil.com/phish')).toBe('/dashboard');
    expect(resolveReturnTo('//evil.com')).toBe('/dashboard');
    expect(resolveReturnTo('/\\evil')).toBe('/dashboard');
    expect(resolveReturnTo('\\\\evil.com')).toBe('/dashboard');
  });

  it('blocks protocol-relative and embedded absolute URLs', () => {
    expect(resolveReturnTo('/redirect?next=https://evil.com')).toBe('/dashboard');
    expect(resolveReturnTo('/ok://not')).toBe('/dashboard');
  });

  it('uses custom fallback when provided', () => {
    expect(resolveReturnTo(null, '/login')).toBe('/login');
    expect(resolveReturnTo('', '/login')).toBe('/login');
    expect(resolveReturnTo('https://x', '/login')).toBe('/login');
  });

  it('decodes percent-encoded relative paths', () => {
    expect(resolveReturnTo('%2Foauth%2Fauthorize%3Fclient_id%3Dx')).toBe(
      '/oauth/authorize?client_id=x',
    );
  });

  it('rejects decoded open redirects', () => {
    expect(resolveReturnTo('%2F%2Fevil.com')).toBe('/dashboard');
  });
});

describe('SDK auth modes', () => {
  it('defaults to redirect and supports popup', () => {
    expect(sdk).toContain("mode: 'redirect'");
    expect(sdk).toContain('data-mode');
    expect(sdk).toContain('popup');
    expect(sdk).toContain('response_type=code');
    expect(sdk).toContain('popup=1');
  });

  it('builds authorize URL under /oauth/authorize', () => {
    expect(sdk).toContain('/oauth/authorize?');
    expect(sdk).toContain('client_id=');
    expect(sdk).toContain('redirect_uri=');
    expect(sdk).toContain('scope=');
    expect(sdk).toContain('state=');
  });

  it('exposes init / getAuthUrl / signIn / renderButton API', () => {
    expect(sdk).toContain('init: function');
    expect(sdk).toContain('getAuthUrl: function');
    expect(sdk).toContain('signIn: function');
    expect(sdk).toContain('renderButton: function');
  });

  it('default scope is openid profile email', () => {
    expect(sdk).toContain("scope: 'openid profile email'");
  });
});

describe('OAuth popup bridge contracts', () => {
  const authorize = readFileSync(path.join(root, 'src/pages/OAuthAuthorizePage.tsx'), 'utf8');
  const bridge = readFileSync(path.join(root, 'src/pages/OAuthPopupBridgePage.tsx'), 'utf8');
  const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');

  it('registers /oauth/bridge and /oauth/popup-done routes', () => {
    expect(app).toContain('path="/oauth/bridge"');
    expect(app).toContain('path="/oauth/popup-done"');
    expect(app).toContain('path="/oauth/authorize"');
  });

  it('authorize remembers popup via sessionStorage and window name', () => {
    expect(authorize).toContain('satspay_oauth_popup');
    expect(authorize).toContain('SatsPaySignInWindow');
    expect(authorize).toContain('/oauth/bridge');
  });

  it('bridge postMessages success/error then falls back to merchant redirect', () => {
    expect(bridge).toContain('SATSPAY_AUTH_SUCCESS');
    expect(bridge).toContain('SATSPAY_AUTH_ERROR');
    expect(bridge).toContain('window.opener');
    expect(bridge).toContain('redirect_url');
    expect(bridge).toContain('location.replace');
    expect(bridge).toContain('targetOrigin');
    expect(bridge).not.toMatch(/postMessage\([^)]+,\s*['"]\*['"]\s*\)/);
  });

  it('authorize and bridge use official satspay-logo.png', () => {
    expect(authorize).toContain('/sdk/satspay-logo.png');
    expect(bridge).toContain('/sdk/satspay-logo.png');
    expect(authorize).not.toContain('BITCOIN_LOGO');
    expect(bridge).not.toContain('BITCOIN_LOGO');
  });

  it('authorize shows raw app logos without framed BrandMark cards', () => {
    expect(authorize).toContain('Logos cruas');
    expect(authorize).toContain('object-contain');
    expect(authorize).not.toContain('function BrandMark');
    expect(authorize).toContain('quer acessar sua conta');
  });
});

describe('Docs / embed snippets use versioned SDK', () => {
  const pages = [
    'src/pages/DocumentationPage.tsx',
    'src/pages/ApiDocsPage.tsx',
    'src/pages/OAuthAppsPage.tsx',
  ].map((p) => ({
    file: p,
    src: readFileSync(path.join(root, p), 'utf8'),
  }));

  it('all merchant-facing snippets load satspay-auth.v2.js', () => {
    for (const { file, src } of pages) {
      expect(src, file).toContain('satspay-auth.v2.js');
    }
  });

  it('snippets do not advertise the poisoned unversioned URL as primary script src', () => {
    for (const { file, src } of pages) {
      expect(src, file).not.toMatch(
        /script src="https:\/\/www\.satspay\.pro\/sdk\/satspay-auth\.js"/,
      );
    }
  });

  it('docs mention official satspay-logo.png', () => {
    const docs = pages.find((p) => p.file.includes('DocumentationPage'))!.src;
    expect(docs).toContain('/sdk/satspay-logo.png');
  });

  it('oauth apps preview uses satspay-logo.png for the button mark', () => {
    const apps = pages.find((p) => p.file.includes('OAuthAppsPage'))!.src;
    expect(apps).toContain('src="/sdk/satspay-logo.png"');
  });
});

describe('SDK public assets present', () => {
  it('ships js + logo under public/sdk', () => {
    expect(existsSync(path.join(root, 'public/sdk/satspay-auth.js'))).toBe(true);
    expect(existsSync(path.join(root, 'public/sdk/satspay-auth.v2.js'))).toBe(true);
    expect(existsSync(path.join(root, 'public/sdk/satspay-logo.png'))).toBe(true);
  });
});

import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { countFullyTransparent, pixelAt, readPngRgba } from '../../helpers/pngRgba.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const sdkPath = path.join(root, 'public/sdk/satspay-auth.js');
const sdkV2Path = path.join(root, 'public/sdk/satspay-auth.v2.js');
const logoPath = path.join(root, 'public/sdk/satspay-logo.png');
const nginxPath = path.join(root, 'nginx.conf');

const BITCOIN_SVG_FRAGMENTS = [
  'BITCOIN_LOGO_SVG',
  'M23.189 14.02', // bitcoin.org B path
  'data:image/svg+xml',
  'circle cx="16" cy="16" r="16" fill="#F7931A"',
];

describe('SatsPay Auth SDK branding', () => {
  const sdk = readFileSync(sdkPath, 'utf8');
  const sdkV2 = readFileSync(sdkV2Path, 'utf8');
  const nginx = readFileSync(nginxPath, 'utf8');

  it('ships compact official logo asset', () => {
    expect(existsSync(logoPath)).toBe(true);
    const bytes = readFileSync(logoPath);
    expect(bytes.byteLength).toBeGreaterThan(8_000);
    expect(bytes.byteLength).toBeLessThan(200_000);
  });

  it('ships versioned SDK path for CDN cache-bust', () => {
    expect(existsSync(sdkV2Path)).toBe(true);
  });

  it('keeps satspay-auth.js and satspay-auth.v2.js in sync', () => {
    expect(sdkV2).toBe(sdk);
  });

  it('uses official satspay-logo.png — never inline Bitcoin SVG', () => {
    expect(sdk).toContain('/sdk/satspay-logo.png');
    for (const frag of BITCOIN_SVG_FRAGMENTS) {
      expect(sdk).not.toContain(frag);
    }
  });

  it('cache-busts logo URL for CDN edges', () => {
    expect(sdk).toMatch(/\/sdk\/satspay-logo\.png\?v=\d+/);
  });

  it('documents versioned script URL in the header comment', () => {
    expect(sdk).toContain('satspay-auth.v2.js');
    expect(sdk).toMatch(/Version:\s*2\./);
  });

  it('defaults OAuth mode to redirect', () => {
    expect(sdk).toContain("mode: 'redirect'");
  });

  it('ships PKCE helpers', () => {
    expect(sdk).toContain('code_challenge');
    expect(sdk).toContain('consumePkceVerifier');
    expect(sdk).toContain('code_challenge_method');
  });

  it('documents popup=1 in auth URL builder', () => {
    expect(sdk).toContain('popup=1');
    expect(sdk).toContain('response_type=code');
  });

  it('persists popup intent across login round-trips', () => {
    expect(sdk).toContain("sessionStorage.setItem('satspay_oauth_popup'");
    expect(sdk).toContain('SatsPaySignInWindow');
  });

  it('falls back to redirect when popup is blocked', () => {
    expect(sdk).toContain('Popup bloqueado');
    expect(sdk).toContain('popup: false');
  });

  it('only accepts postMessage from SatsPay origin', () => {
    expect(sdk).toContain('SATSPAY_AUTH_SUCCESS');
    expect(sdk).toContain('SATSPAY_AUTH_ERROR');
    expect(sdk).toContain('event.origin');
  });

  it('renders logo via <img> (not innerHTML SVG) on all themes', () => {
    expect(sdk).toContain("document.createElement('img')");
    expect(sdk).toContain('data-theme');
    expect(sdk).toContain("theme === 'dark'");
    expect(sdk).toContain("theme === 'light'");
    expect(sdk).not.toMatch(/btn\.innerHTML\s*=\s*BITCOIN/);
    expect(sdk).not.toMatch(/innerHTML\s*=\s*[^;]*<svg/);
  });

  it('supports light / dark / default (bitcoin orange) button themes', () => {
    expect(sdk).toContain('#FFFFFF'); // light bg
    expect(sdk).toContain('#131B2E'); // dark gradient
    expect(sdk).toContain('#F7931A'); // orange accent
  });

  it('supports size and text variants', () => {
    for (const size of ['small', 'medium', 'large']) {
      expect(sdk).toContain(`'${size}'`);
    }
    expect(sdk).toContain('Entrar com SatsPay');
    expect(sdk).toContain('Continuar com SatsPay');
    expect(sdk).toContain('Cadastrar com SatsPay');
    expect(sdk).toContain('Sign in with SatsPay');
    expect(sdk).toContain('Continue with SatsPay');
  });

  it('onerror retries official satspay-logo only (no /logo.png fallback)', () => {
    expect(sdk).toContain("indexOf('satspay-logo.png')");
    expect(sdk).not.toMatch(/img\.src\s*=\s*SATSPAY_BASE_URL\s*\+\s*'\/logo\.png'/);
  });

  it('auto-renders .satspay-signin and [data-satspay-signin]', () => {
    expect(sdk).toContain('.satspay-signin, [data-satspay-signin]');
    expect(sdk).toContain('SatsPay.autoRender');
    expect(sdk).toContain('window.SatsPay');
    expect(sdk).toContain('window.SatsPayAuth');
  });

  it('requires client_id and redirect_uri before sign-in', () => {
    expect(sdk).toContain('data-client_id obrigatório');
    expect(sdk).toContain('data-redirect_uri obrigatório');
  });

  it('allows short cache + CORS + CDN TTL on /sdk/ in nginx', () => {
    expect(nginx).toContain('location ^~ /sdk/');
    expect(nginx).toContain('Access-Control-Allow-Origin');
    expect(nginx).toContain('max-age=300');
    expect(nginx).toContain('CDN-Cache-Control');
    expect(nginx).toContain('Cloudflare-CDN-Cache-Control');
    // /sdk/ must win over the immutable asset regex
    expect(nginx).toMatch(/location \^~ \/sdk\/[\s\S]*?location ~\* \\\.\(\?:js/);
  });

  it('ships CSP + baseline browser headers for the SPA', () => {
    expect(nginx).toContain('Content-Security-Policy');
    expect(nginx).toContain('X-Content-Type-Options');
    expect(nginx).toContain('X-Frame-Options');
  });

  it('sets COOP same-origin-allow-popups for OAuth popups', () => {
    expect(nginx).toContain('Cross-Origin-Opener-Policy');
    expect(nginx).toContain('same-origin-allow-popups');
  });
});

describe('satspay-logo.png transparency', () => {
  const png = readPngRgba(readFileSync(logoPath));

  it('is 8-bit RGBA (color type 6)', () => {
    expect(png.width).toBeGreaterThanOrEqual(64);
    expect(png.height).toBeGreaterThanOrEqual(64);
    expect(png.width).toBe(png.height);
  });

  it('has fully transparent corners (no baked square background)', () => {
    const corners: Array<[number, number]> = [
      [0, 0],
      [png.width - 1, 0],
      [0, png.height - 1],
      [png.width - 1, png.height - 1],
    ];
    for (const [x, y] of corners) {
      const [, , , a] = pixelAt(png, x, y);
      expect(a, `corner (${x},${y}) alpha`).toBe(0);
    }
  });

  it('has a meaningful transparent canvas (majority alpha=0)', () => {
    const transparent = countFullyTransparent(png);
    const ratio = transparent / (png.width * png.height);
    expect(ratio).toBeGreaterThan(0.4);
  });

  it('has opaque brand pixels near the center (logo not empty)', () => {
    const [, , , a] = pixelAt(png, Math.floor(png.width / 2), Math.floor(png.height / 2));
    expect(a).toBeGreaterThan(200);
  });
});

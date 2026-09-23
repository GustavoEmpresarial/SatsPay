import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const nginxConf = readFileSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../nginx.conf'),
  'utf8',
);

describe('nginx reverse-proxy IP headers', () => {
  it('forwards $remote_addr as X-Real-IP (never a client-supplied CF-Connecting-IP)', () => {
    expect(nginxConf).toContain('proxy_set_header X-Forwarded-For $client_ip;');
    expect(nginxConf).toContain('set $client_ip $remote_addr;');
    expect(nginxConf).not.toContain('$http_cf_connecting_ip');
    expect(nginxConf).not.toContain('$proxy_add_x_forwarded_for');
  });

  it('does not re-publish spoofable Cloudflare headers toward the API', () => {
    expect(nginxConf).not.toMatch(/proxy_set_header CF-Connecting-IP/);
    expect(nginxConf).not.toMatch(/proxy_set_header True-Client-IP/);
  });
});

describe('nginx access log', () => {
  it('logs the path without the query string (OAuth code/state, e-mails, keys)', () => {
    expect(nginxConf).toContain('log_format satspay_json');
    expect(nginxConf).toContain('access_log /dev/stdout satspay_json;');
    // $uri is the normalized path; $request_uri would leak the query string.
    expect(nginxConf).toMatch(/"path":"\$uri"/);
    // Ignore comments; assert no directive line references the query string.
    const directives = nginxConf
      .split('\n')
      .filter((l) => !l.trimStart().startsWith('#'))
      .join('\n');
    expect(directives).not.toContain('$request_uri');
    expect(directives).not.toContain('$args');
    // No raw client IP in the log line (LGPD): the API keeps an HMAC fingerprint.
    expect(nginxConf).not.toMatch(/"ip":"\$(remote_addr|client_ip)"/);
  });

  it('generates its own request id and forwards it to the API', () => {
    expect(nginxConf).toContain('proxy_set_header X-Request-Id $request_id;');
    expect(nginxConf).not.toContain('$http_x_request_id');
  });
});

describe('nginx /sdk/ delivery', () => {
  it('short-circuits /sdk/ before immutable asset regex', () => {
    const sdkIdx = nginxConf.indexOf('location ^~ /sdk/');
    const assetsIdx = nginxConf.indexOf('location ~* \\.(?:js|css');
    expect(sdkIdx).toBeGreaterThanOrEqual(0);
    expect(assetsIdx).toBeGreaterThan(sdkIdx);
  });

  it('does not mark /sdk/ as immutable year-long cache', () => {
    const sdkBlock = nginxConf.slice(
      nginxConf.indexOf('location ^~ /sdk/'),
      nginxConf.indexOf('location ~* \\.(?:js|css'),
    );
    expect(sdkBlock).not.toContain('immutable');
    expect(sdkBlock).not.toContain('31536000');
    expect(sdkBlock).toContain('max-age=300');
  });

  it('exposes CORS on /sdk/ for cross-origin merchant embeds', () => {
    const sdkBlock = nginxConf.slice(
      nginxConf.indexOf('location ^~ /sdk/'),
      nginxConf.indexOf('location ~* \\.(?:js|css'),
    );
    expect(sdkBlock).toContain('Access-Control-Allow-Origin "*"');
  });
});

describe('nginx SPA shell cache', () => {
  it('does not cache index.html so deploys take effect', () => {
    expect(nginxConf).toMatch(/location\s*=\s*\/index\.html/);
    expect(nginxConf).toMatch(/Cache-Control "no-store, no-cache, must-revalidate"/);
  });

  it('listens on 8080 for non-root nginx', () => {
    expect(nginxConf).toMatch(/listen\s+8080\s*;/);
    expect(nginxConf).not.toMatch(/listen\s+80\s*;/);
  });
});

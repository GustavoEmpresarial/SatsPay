import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const nginxConf = readFileSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../nginx.conf'),
  'utf8',
);

describe('nginx reverse-proxy IP headers', () => {
  it('forwards a resolved client IP (CF-Connecting-IP preferred over docker $remote_addr)', () => {
    expect(nginxConf).toContain('proxy_set_header X-Forwarded-For $client_ip;');
    expect(nginxConf).toContain('$http_cf_connecting_ip');
    expect(nginxConf).not.toContain('$proxy_add_x_forwarded_for');
  });

  it('does not re-publish spoofable Cloudflare headers toward the API', () => {
    expect(nginxConf).not.toMatch(/proxy_set_header CF-Connecting-IP/);
    expect(nginxConf).not.toMatch(/proxy_set_header True-Client-IP/);
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
});

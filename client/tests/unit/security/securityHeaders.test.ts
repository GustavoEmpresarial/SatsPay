import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const nginxConf = readFileSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../nginx.conf'),
  'utf8',
);

/** Security headers / CSP / clickjacking (#10). */
describe('security headers (nginx)', () => {
  it('sets baseline browser hardening headers', () => {
    expect(nginxConf).toContain('X-Content-Type-Options "nosniff"');
    expect(nginxConf).toContain('X-Frame-Options "DENY"');
    expect(nginxConf).toContain('Referrer-Policy "strict-origin-when-cross-origin"');
    expect(nginxConf).toContain('Permissions-Policy');
    expect(nginxConf).toContain('Content-Security-Policy');
  });

  it('CSP blocks framing and inline object embeds', () => {
    expect(nginxConf).toMatch(/frame-ancestors 'none'/);
    expect(nginxConf).toMatch(/object-src 'none'/);
    expect(nginxConf).toMatch(/base-uri 'self'/);
  });

  it('does not allow wildcard script-src', () => {
    const cspMatch = nginxConf.match(/Content-Security-Policy "([^"]+)"/);
    expect(cspMatch?.[1]).toBeTruthy();
    const csp = cspMatch![1];
    expect(csp).not.toMatch(/script-src[^;]*\*/);
    expect(csp).toContain("script-src 'self'");
  });
});

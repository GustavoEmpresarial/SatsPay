import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const authStore = readFileSync(path.join(root, 'src/stores/auth.ts'), 'utf8');
const adminStore = readFileSync(path.join(root, 'src/stores/admin.ts'), 'utf8');
const api = readFileSync(path.join(root, 'src/lib/api.ts'), 'utf8');

describe('session token storage (XSS blast-radius reduction)', () => {
  it('auth persist partialize keeps only user — never access/refresh tokens', () => {
    expect(authStore).toContain("name: 'bitcosats-auth'");
    expect(authStore).toMatch(/partialize:\s*\(state\).*\{ user: state\.user \}/s);
    expect(authStore).not.toMatch(/partialize:[\s\S]*accessToken:\s*state\.accessToken/);
    expect(authStore).not.toMatch(/partialize:[\s\S]*refreshToken:\s*state\.refreshToken/);
    expect(authStore).toContain('version: 2');
  });

  it('admin persist partialize keeps only admin profile', () => {
    expect(adminStore).toContain("name: 'bitcosats-admin'");
    expect(adminStore).toMatch(/partialize:\s*\(state\).*\{ admin: state\.admin \}/s);
    expect(adminStore).not.toMatch(/partialize:[\s\S]*accessToken:\s*state\.accessToken/);
    expect(adminStore).toContain('version: 2');
  });

  it('refresh uses cookie credentials with empty body (no token from JS storage)', () => {
    expect(api).toContain("body: '{}'");
    expect(api).toContain("credentials: 'include'");
    expect(api).toContain('bootstrapSession');
    expect(api).not.toMatch(/refreshToken\s*=\s*useAuthStore\.getState\(\)\.refreshToken/);
  });
});

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const page = readFileSync(path.join(root, 'src/pages/LoginPage.tsx'), 'utf8');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');

describe('LoginPage structure', () => {
  it('is routed at /login', () => {
    expect(app).toContain('path="/login"');
    expect(app).toContain('LoginPage');
  });

  it('posts to /auth/login with captcha and optional OTP', () => {
    expect(page).toContain("'/auth/login'");
    expect(page).toContain('LOGIN_CAPTCHA_ACTION');
    expect(page).toContain('emailCode');
    expect(page).toContain('codeSent');
    expect(page).toContain('resolveReturnTo');
    expect(page).toContain('reportAuthFailure');
  });

  it('links to register and does not persist password', () => {
    expect(page).toContain('to="/register"');
    expect(page).not.toMatch(/localStorage\.setItem\([^)]*password/);
  });
});

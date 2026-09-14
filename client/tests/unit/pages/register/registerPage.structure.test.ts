import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const page = readFileSync(path.join(root, 'src/pages/RegisterPage.tsx'), 'utf8');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');

describe('RegisterPage structure', () => {
  it('is routed at /register', () => {
    expect(app).toContain('path="/register"');
    expect(app).toContain('RegisterPage');
  });

  it('uses shared authValidation and register captcha', () => {
    expect(page).toContain("from '../lib/authValidation.js'");
    expect(page).toContain('REGISTER_CAPTCHA_ACTION');
    expect(page).toContain("'/auth/register'");
    expect(page).toContain('acceptTerms');
    expect(page).toContain('referralCode');
  });

  it('requires terms for submit button', () => {
    expect(page).toMatch(/disabled=\{loading \|\| !acceptTerms\}/);
  });
});

describe('Referral deep link', () => {
  it('maps /r/:code to register?r=', () => {
    expect(app).toContain('path="/r/:code"');
    expect(app).toContain('ReferralRedirect');
    expect(app).toMatch(/Navigate to=\{`\/register\?r=/);
  });
});

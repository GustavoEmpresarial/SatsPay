import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/SettingsPage.tsx');

describe('Settings — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/settings\"");
    expect(app).toContain("SettingsPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
    expect(src).toContain('/me/export');
    expect(src).toContain('/me/erase');
  });

  it('erase uses session only — confirmEmail + APAGAR, no user_id', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('confirmEmail: eraseEmail');
    expect(src).toContain('confirm: erasePhrase');
    const eraseCall = src.slice(src.indexOf("api('/me/erase'"), src.indexOf("api('/me/erase'") + 280);
    expect(eraseCall).not.toMatch(/user_id|userId/);
    const handler = readFileSync(path.join(root, '../crates/api-http/src/auth.rs'), 'utf8');
    const erase = handler.slice(handler.indexOf('async fn erase_me'));
    expect(erase).toContain('AuthUser');
    expect(erase).toContain('confirm_email');
    expect(erase).toContain('APAGAR');
    expect(erase).toContain('user.id');
    expect(erase).not.toMatch(/struct EraseRequest[\s\S]{0,200}user_id/);
  });
});

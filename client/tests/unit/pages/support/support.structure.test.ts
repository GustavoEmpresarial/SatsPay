import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/SupportPage.tsx');
const adminPagePath = path.join(root, 'src/pages/AdminSupportPage.tsx');
const adminLayout = readFileSync(path.join(root, 'src/components/AdminLayout.tsx'), 'utf8');
const pt = JSON.parse(readFileSync(path.join(root, 'src/i18n/locales/pt.json'), 'utf8'));
const en = JSON.parse(readFileSync(path.join(root, 'src/i18n/locales/en.json'), 'utf8'));

describe('Support — structure (interno, sem email)', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain('path="/support"');
    expect(app).toContain('SupportPage');
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function SupportPage/);
  });

  it('uses in-app tickets API — not mailto', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain("/support/tickets");
    expect(src).not.toMatch(/mailto:/);
    expect(src).not.toContain('support@satspay.pro');
    expect(src).toContain('POST');
  });

  it('covers create + list + reply flows', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('/support/tickets');
    expect(src).toContain('/messages');
    expect(src).toContain('myTickets');
  });

  it('i18n support keys exist in pt and en without mail CTA', () => {
    for (const loc of [pt, en]) {
      expect(loc.support?.send).toBeTruthy();
      expect(loc.support?.myTickets).toBeTruthy();
      expect(String(loc.support?.send).toLowerCase()).not.toContain('email');
      expect(loc.support?.mail).toBeUndefined();
      expect(loc.support?.topics?.deposit).toBeTruthy();
      expect(loc.support?.status?.OPEN).toBeTruthy();
    }
  });
});

describe('Admin Support — structure', () => {
  it('admin page exists and is routed', () => {
    expect(existsSync(adminPagePath)).toBe(true);
    expect(app).toContain('path="support"');
    expect(app).toContain('AdminSupportPage');
  });

  it('nav includes Suporte', () => {
    expect(adminLayout).toContain("/admin/support");
    expect(adminLayout).toContain('Suporte');
  });

  it('calls admin support APIs', () => {
    const src = readFileSync(adminPagePath, 'utf8');
    expect(src).toContain('/admin/support/tickets');
    expect(src).toContain('/messages');
    expect(src).toContain('/status');
    expect(src).toMatch(/export function AdminSupportPage/);
  });
});

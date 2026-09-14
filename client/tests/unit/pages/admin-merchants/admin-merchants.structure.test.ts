import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/AdminMerchantsPage.tsx');

describe('Admin Merchants — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"merchants\"");
    expect(app).toContain("AdminMerchantsPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('has Overview + Stats + Merchants tabs and stats endpoint', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('/admin/merchants/stats');
    expect(src).toMatch(/overview/);
    expect(src).toMatch(/Estatísticas/);
    expect(src).toMatch(/Comerciantes/);
    expect(src).toMatch(/series_14d|Atividade 14 dias/);
    expect(src).toMatch(/Funil de faturas/);
    expect(src).not.toMatch(/Ecossistema SatsPay Merchant/);
    expect(src).not.toMatch(/Como funciona o Gateway/);
  });
});

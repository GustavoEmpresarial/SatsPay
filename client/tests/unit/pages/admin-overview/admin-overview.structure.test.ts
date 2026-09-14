import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/AdminOverviewPage.tsx');

describe('Admin Overview — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/admin\"");
    expect(app).toContain("AdminOverviewPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('has Economia tab and economics endpoint', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('/admin/economics');
    expect(src).toMatch(/Economia/);
    expect(src).toMatch(/Receita gateway|Gateway de depósito/);
    expect(src).toMatch(/Faucet/);
    expect(src).toMatch(/Margem de taxas/);
    expect(src).toMatch(/fee_margin_by_coin/);
  });
});

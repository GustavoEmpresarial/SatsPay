import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/AdminStakePage.tsx');

describe('Admin Stake — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"stake\"");
    expect(app).toContain("AdminStakePage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('shows simplified treasury columns', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('/admin/economics');
    expect(src).toContain('/admin/treasury-health');
    expect(src).toMatch(/Nossa \(hot\)|Nossa carteira/);
    expect(src).toMatch(/Depósitos/);
    expect(src).toMatch(/Saques/);
    expect(src).toMatch(/Saúde financeira/);
    expect(src).toMatch(/Margem de taxas/);
    expect(src).toMatch(/P&L \(USD\)|Resultado \(USD\)/);
    expect(src).toMatch(/Break-even|Equilíbrio do saque/);
    expect(src).toMatch(/Runway HOUSE|Autonomia da HOUSE/);
    expect(src).toMatch(/Passivos pendentes/);
    expect(src).toMatch(/Sweeps parados|Varreduras pendentes/);
    expect(src).toMatch(/Buffer hot|Reserva da hot/);
    expect(src).toMatch(/Trava margem|Trava de margem/);
    expect(src).toMatch(/Taxas × rede/);
    expect(src).not.toMatch(/RPM|Latência avg|Postgres/);
    expect(src).toContain('/admin');
  });
});

import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/MerchantDashboardPage.tsx');

describe('Merchant Dashboard — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/merchant\"");
    expect(app).toContain("path=\"/merchant/dashboard\"");
    expect(app).toContain("MerchantDashboardPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('converts ledger amounts with formatLedgerAmount / getCoinUsdValue (not raw×price)', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('formatLedgerAmount');
    expect(src).toContain('getCoinUsdValue');
    expect(src).toContain('priceUsdScaled');
    expect(src).not.toMatch(/Number\(inv\.amount\)\s*\*\s*price/);
  });
});

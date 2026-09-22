import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/SwapPage.tsx');

describe('Swap — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/swap\"");
    expect(app).toContain("SwapPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('refreshes wallets when an in-flight swap settles', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('prevSwapStatusRef');
    expect(src).toContain('isSwapInFlight');
    expect(src).toContain('isSwapTerminal');
    expect(src).toContain("invalidateQueries({ queryKey: ['wallets'] })");
  });

  it('shows custodial network on coin picker', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('coinNetwork');
    expect(src).toContain('swap.networkOn');
  });

  it('separates Swap and Bridge tabs', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain("tab') === 'bridge'");
    expect(src).toContain('switchMode');
    expect(src).toContain('isDexSwapPair');
    expect(src).toContain('isBridgePair');
    expect(src).toContain('tabSwap');
    expect(src).toContain('tabBridge');
  });

  it('shows a friendly empty state when quote has no routes', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('noRoutes');
    expect(src).toContain('swap.noRoutesTitle');
    expect(src).toContain('isNoRoutesError');
  });

  it('explains a refund/failure in history instead of a bare status pill', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('explainSwapError');
    // Raw provider/RPC text belongs in support tooling, not the user-facing list.
    expect(src).not.toMatch(/\{s\.error\}/);
  });

  it('warns when route costs eat a large share of the value', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('valueLossWarning');
    expect(src).toContain('swap.valueLossTitle');
    expect(src).toContain('swap.valueLossBody');
    // Compares USD in vs USD out, so it catches any provider, not just ChangeNOW.
    expect(src).toContain('fromUsdNum');
    expect(src).toContain('toUsdNum');
  });
});

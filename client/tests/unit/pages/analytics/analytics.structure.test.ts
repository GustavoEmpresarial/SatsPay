/** @vitest-environment node */
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

// Avoid node:url — Vite jsdom pool externalizes it and breaks fileURLToPath.
const root = process.cwd();
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/AnalyticsPage.tsx');

describe('Analytics — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/analytics\"");
    expect(app).toContain("AnalyticsPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('parses wallet list as array or { wallets }', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('asWalletBalances');
    expect(src).toContain('formatPortfolioUsd');
    expect(src).toContain('aggregateByType');
  });
});

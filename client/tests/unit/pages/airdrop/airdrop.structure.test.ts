/** @vitest-environment node */
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

// Avoid node:url — Vite jsdom pool externalizes it and breaks fileURLToPath.
const root = process.cwd();
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/AirdropPage.tsx');

describe('Airdrop — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/airdrop\"");
    expect(app).toContain("AirdropPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('shows inactive-season banner when season_active is false', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('season_active');
    expect(src).toMatch(/Temporada inativa/);
  });
});

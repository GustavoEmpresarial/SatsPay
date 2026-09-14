import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/DocumentationPage.tsx');
const statusPath = path.join(root, 'src/pages/StatusPage.tsx');
const apiDocsPath = path.join(root, 'src/pages/ApiDocsPage.tsx');

const FORBIDDEN = [
  /FOR UPDATE/i,
  /ledger_entries/,
  /\bHOUSE\b/,
  /captcha_seen_tokens/,
  /\bxpub\b/i,
  /AES-256-GCM/i,
  /\bHKDF\b/,
  /\bPITR\b/,
  /\bWAL\b/,
  /node VM/i,
  /169\.58\./,
  /62\.171\./,
  /contato\.pessoal\.010@gmail\.com/,
  /ENCRYPTION_KEY/,
  /HOT_MNEMONIC|BTC_HOT_WIF|DATABASE_URL/i,
];

describe('Documentation (in-app) — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain('path="/documentation"');
    expect(app).toContain('DocumentationPage');
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('does not publish infra secrets or internal ops detail', () => {
    const docs = readFileSync(pagePath, 'utf8');
    const status = readFileSync(statusPath, 'utf8');
    const api = readFileSync(apiDocsPath, 'utf8');
    const blob = `${docs}\n${status}\n${api}`;
    for (const re of FORBIDDEN) {
      expect(blob, `forbidden pattern ${re}`).not.toMatch(re);
    }
  });
});

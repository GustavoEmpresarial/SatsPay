import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const pagesDir = path.join(root, 'src/pages');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');

describe('site-wide page inventory', () => {
  const pageFiles = readdirSync(pagesDir).filter((f) => f.endsWith('Page.tsx'));

  it('has page modules on disk', () => {
    expect(pageFiles.length).toBeGreaterThanOrEqual(40);
  });

  it('every *Page.tsx is referenced from App.tsx', () => {
    const missing: string[] = [];
    for (const file of pageFiles) {
      const name = file.replace(/\.tsx$/, '');
      if (!app.includes(name)) missing.push(name);
    }
    expect(missing, `Unwired pages: ${missing.join(', ')}`).toEqual([]);
  });

  it('docs/pages covers each critical slug folder', () => {
    const docsPages = path.join(root, '../docs/pages');
    const required = [
      'landing',
      'login',
      'register',
      'dashboard',
      'wallets',
      'deposit',
      'withdraw',
      'faucet',
      'swap',
      'admin-login',
    ];
    for (const slug of required) {
      expect(readdirSync(path.join(docsPages, slug))).toContain('README.md');
    }
  });
});

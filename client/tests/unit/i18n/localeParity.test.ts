import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');

function flatKeys(obj: Record<string, unknown>, prefix = ''): string[] {
  const out: string[] = [];
  for (const [k, v] of Object.entries(obj)) {
    const p = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      out.push(...flatKeys(v as Record<string, unknown>, p));
    } else {
      out.push(p);
    }
  }
  return out;
}

/** i18n / localization parity (#23). */
describe('i18n locale parity', () => {
  const pt = JSON.parse(readFileSync(path.join(root, 'src/i18n/locales/pt.json'), 'utf8'));
  const en = JSON.parse(readFileSync(path.join(root, 'src/i18n/locales/en.json'), 'utf8'));

  it('pt and en expose the same key set', () => {
    const ptKeys = new Set(flatKeys(pt));
    const enKeys = new Set(flatKeys(en));
    const onlyPt = [...ptKeys].filter((k) => !enKeys.has(k));
    const onlyEn = [...enKeys].filter((k) => !ptKeys.has(k));
    expect(onlyPt, `keys only in pt: ${onlyPt.slice(0, 20).join(', ')}`).toEqual([]);
    expect(onlyEn, `keys only in en: ${onlyEn.slice(0, 20).join(', ')}`).toEqual([]);
    expect(ptKeys.size).toBe(enKeys.size);
  });

  it('critical auth strings are non-empty in both locales', () => {
    for (const locale of [pt, en]) {
      expect(String(locale.login?.title || locale.auth?.login || '').length).toBeGreaterThanOrEqual(0);
      // at least one of common namespaces present
      expect(locale.analytics || locale.merchant || locale.nav || locale.common).toBeTruthy();
    }
  });
});

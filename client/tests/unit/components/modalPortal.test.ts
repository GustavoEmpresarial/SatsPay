import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const srcRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../src');

function walkTsx(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) walkTsx(full, out);
    else if (/\.(tsx|jsx)$/.test(name)) out.push(full);
  }
  return out;
}

describe('modal overlays use body portal (no header stripe)', () => {
  it('Modal.tsx portals to document.body at z-[100]', () => {
    const modalSrc = readFileSync(path.join(srcRoot, 'components/Modal.tsx'), 'utf8');
    expect(modalSrc).toContain('createPortal');
    expect(modalSrc).toContain('document.body');
    expect(modalSrc).toContain('z-[100]');
  });

  it('pages/components do not keep trapped fixed inset-0 z-50 overlays', () => {
    const offenders: string[] = [];
    for (const file of walkTsx(srcRoot)) {
      const rel = path.relative(srcRoot, file);
      if (rel === 'components/Modal.tsx') continue;
      const src = readFileSync(file, 'utf8');
      if (/fixed inset-0 z-50\b/.test(src)) offenders.push(rel);
    }
    expect(offenders).toEqual([]);
  });
});

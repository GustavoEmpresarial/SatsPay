import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const srcRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../src');

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else if (/\.(ts|tsx)$/.test(name)) out.push(full);
  }
  return out;
}

/** Architecture boundary tests (#30). */
describe('architecture boundaries', () => {
  const files = walk(srcRoot);

  it('shared/ does not import React or pages', () => {
    const sharedFiles = files.filter((f) => f.includes(`${path.sep}shared${path.sep}`));
    for (const file of sharedFiles) {
      const src = readFileSync(file, 'utf8');
      expect(src, file).not.toMatch(/from ['"]react['"]/);
      expect(src, file).not.toMatch(/from ['"].*\/pages\//);
    }
  });

  it('lib/ does not import pages/', () => {
    const libFiles = files.filter((f) => f.includes(`${path.sep}lib${path.sep}`));
    for (const file of libFiles) {
      const src = readFileSync(file, 'utf8');
      expect(src, file).not.toMatch(/from ['"].*pages\//);
    }
  });

  it('pages do not reach into crates or node_modules via relative ../../../../', () => {
    const pageFiles = files.filter((f) => f.includes(`${path.sep}pages${path.sep}`));
    for (const file of pageFiles) {
      const src = readFileSync(file, 'utf8');
      expect(src, file).not.toMatch(/from ['"]\.\.\/\.\.\/\.\.\/\.\.\//);
    }
  });
});

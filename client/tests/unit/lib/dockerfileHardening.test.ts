import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const dockerfile = readFileSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../Dockerfile'),
  'utf8',
);

describe('client Dockerfile hardening', () => {
  it('uses unprivileged nginx base (non-root USER baked in)', () => {
    expect(dockerfile).toMatch(/FROM\s+nginxinc\/nginx-unprivileged/);
    expect(dockerfile).toMatch(/EXPOSE\s+8080/);
  });
});

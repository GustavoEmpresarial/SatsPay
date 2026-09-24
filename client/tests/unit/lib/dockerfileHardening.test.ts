import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const dockerfile = readFileSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../Dockerfile'),
  'utf8',
);
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const compose = readFileSync(path.join(repoRoot, 'deploy/docker/docker-compose.yml'), 'utf8');
const composeEnvExample = readFileSync(path.join(repoRoot, 'deploy/docker/.env.example'), 'utf8');

describe('client Dockerfile hardening', () => {
  it('uses unprivileged nginx base (non-root USER baked in)', () => {
    expect(dockerfile).toMatch(/FROM\s+nginxinc\/nginx-unprivileged/);
    expect(dockerfile).toMatch(/EXPOSE\s+8080/);
  });

  it('publishes the client only on loopback by default', () => {
    expect(compose).toContain('${CLIENT_HOST_BIND:-127.0.0.1}:${CLIENT_HOST_PORT:-8080}:8080');
    expect(compose).not.toContain('${CLIENT_HOST_BIND:-0.0.0.0}');
    expect(composeEnvExample).toContain('CLIENT_HOST_BIND=127.0.0.1');
  });
});

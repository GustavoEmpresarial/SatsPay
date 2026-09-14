import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const compose = readFileSync(path.join(root, 'deploy/docker/docker-compose.yml'), 'utf8');

describe('secrets hygiene', () => {
  it('compose does not hardcode Turnstile bypass', () => {
    expect(compose).not.toMatch(/TURNSTILE_SECRET:\s*"disabled_in_dev"/);
    expect(compose).toContain('TURNSTILE_SECRET: ${TURNSTILE_SECRET');
  });

  it('compose wires production NODE_ENV + COOKIE_SECURE from env', () => {
    expect(compose).toContain('NODE_ENV: ${NODE_ENV');
    expect(compose).toContain('COOKIE_SECURE');
  });

  it('compose wires SMTP from env (not hardcoded off)', () => {
    expect(compose).not.toMatch(/SMTP_ENABLED:\s*"false"/);
    expect(compose).toContain('SMTP_ENABLED: ${SMTP_ENABLED');
    expect(compose).toContain('SMTP_HOST: ${SMTP_HOST');
    expect(compose).toContain('SMTP_USERNAME: ${SMTP_USERNAME');
    expect(compose).toContain('SMTP_PASSWORD: ${SMTP_PASSWORD');
  });
});

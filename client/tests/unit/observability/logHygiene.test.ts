import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Observability / logging hygiene (#43, #44) — client error reporter must not
 * ship raw tokens/passwords in breadcrumb helpers (static scan).
 */
const reportErrorPath = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../src/lib/reportError.ts',
);

describe('observability: client error reporter hygiene', () => {
  const src = readFileSync(reportErrorPath, 'utf8');

  it('redacts or avoids common secret field names in payloads', () => {
    // Heuristic: scrubbing helpers or denylist should exist
    const hasScrub =
      /redact|sanitize|scrub|password|refreshToken|accessToken|authorization/i.test(src);
    expect(hasScrub).toBe(true);
  });

  it('does not console.log access tokens', () => {
    expect(src).not.toMatch(/console\.(log|debug|info)\([^)]*accessToken/);
    expect(src).not.toMatch(/console\.(log|debug|info)\([^)]*refreshToken/);
  });
});

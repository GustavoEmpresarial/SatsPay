/**
 * Every route the API serves must be documented somewhere, and every route
 * the docs promise must exist.
 *
 * This session started because the published docs described endpoints the
 * server did not have and omitted ones it did. Prose cannot be trusted to
 * stay true; this test makes the gap a build failure.
 */
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const clientRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const repoRoot = path.resolve(clientRoot, '..');
const httpSrc = path.join(repoRoot, 'crates/api-http/src');

const reference = readFileSync(path.join(repoRoot, 'docs/api/http-api-reference.md'), 'utf8');
const publicDocs = readFileSync(path.join(clientRoot, 'src/pages/ApiDocsPage.tsx'), 'utf8');

/** Every `.route("…")` registered by the axum router. */
function declaredRoutes(): string[] {
  const routes = new Set<string>();
  for (const file of readdirSync(httpSrc).filter((f) => f.endsWith('.rs'))) {
    const src = readFileSync(path.join(httpSrc, file), 'utf8');
    for (const m of src.matchAll(/\.route\(\s*"([^"]+)"/g)) routes.add(m[1]);
  }
  return [...routes].sort();
}

/**
 * Several paths are the same handler mounted under legacy prefixes, so the
 * documentation names the canonical one and does not repeat itself.
 */
function canonical(route: string): string {
  return route
    .replace(/^\/public\//, '/v1/public/')
    .replace(/^\/(faucet|faucetlist)(\/|$)/, '/v1/$1$2')
    .replace(/^\/api-keys/, '/v1/api-keys')
    .replace(/^\/v1\/public\/keys/, '/v1/api-keys')
    .replace(/^\/v1\/merchant\/invoices/, '/v1/merchant/deposits')
    .replace(/^\/v1\/merchant\/deposits\/create$/, '/v1/merchant/deposits');
}

/** Infrastructure, not an API surface a reader looks up. */
const NOT_DOCUMENTED = new Set(['/healthz', '/metrics']);

/**
 * Routes that exist for the app's own screens rather than for integrators.
 * They belong in the internal reference, not on the public docs page.
 */
const INTERNAL_ONLY = /^\/v1\/(admin|auth|telemetry|wallet|deposits|withdrawals|faucet|faucetlist|stake|lend|swap|rewards|referral|airdrop|support|status|notify)/;

describe('contract: every route is documented', () => {
  const routes = declaredRoutes().map(canonical);
  const unique = [...new Set(routes)].filter((r) => !NOT_DOCUMENTED.has(r));

  it('finds the router (guards against the extractor silently breaking)', () => {
    expect(unique.length).toBeGreaterThan(50);
    expect(unique).toContain('/v1/merchant/deposits');
  });

  it('documents every route in the internal reference', () => {
    const missing = unique.filter((r) => !reference.includes(r));
    expect(missing, `undocumented routes:\n${missing.join('\n')}`).toEqual([]);
  });

  it('documents every integrator-facing route on the public page', () => {
    const integrator = unique.filter((r) => !INTERNAL_ONLY.test(r));
    const missing = integrator.filter((r) => !publicDocs.includes(r));
    expect(missing, `missing from /docs:\n${missing.join('\n')}`).toEqual([]);
  });
});

describe('contract: the docs promise nothing that does not exist', () => {
  it('every /v1 path cited on the public page is a real route', () => {
    // The original defect ran this way round: the page advertised
    // `POST /v1/merchant/deposits` against a route that did not accept POST.
    const declared = new Set(declaredRoutes());
    const cited = new Set(
      [...publicDocs.matchAll(/\/v1\/[a-z0-9/:_-]+/gi)]
        .map((m) => m[0].replace(/[.,)]+$/, ''))
        // `/v1/merchant/*` in prose names a family, not a path.
        .map((p) => p.replace(/\/$/, ''))
        // Concrete ids in examples stand for the `:id` parameter.
        .map((p) => p.replace(/\/[0-9a-f]{8}-[0-9a-f-]{27,}/i, '/:id'))
        .map((p) => p.replace(/\/550e8400[^/]*/, '/:id')),
    );

    const phantom = [...cited].filter((p) => {
      if (declared.has(p)) return false;
      return ![...declared].some(
        (d) =>
          // The canonical spelling of an alias family.
          canonical(d) === p ||
          d === p ||
          // Prose naming a family rather than one path, e.g. `/v1/merchant/*`.
          d.startsWith(`${p}/`),
      );
    });
    expect(phantom, `documented but not routed:\n${phantom.join('\n')}`).toEqual([]);
  });
});

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

/**
 * Cross-checks the published docs page against the Rust handlers it
 * describes. The two drifted badly enough that documented integrations
 * could not work at all, and nothing in CI noticed. These assertions fail
 * when either side changes a field name without the other.
 */
const clientRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const repoRoot = path.resolve(clientRoot, '..');

const docs = readFileSync(path.join(clientRoot, 'src/pages/ApiDocsPage.tsx'), 'utf8');
const handler = readFileSync(path.join(repoRoot, 'crates/api-http/src/merchant_deposits.rs'), 'utf8');
const webhook = readFileSync(path.join(repoRoot, 'crates/webhooks/src/lib.rs'), 'utf8');

/** Field names of a `json!({ ... })` block starting at `marker`. */
function jsonFields(source: string, marker: string): string[] {
  return fieldsBetween(source, marker, '})');
}

/** Field names of the JSON literal starting at `marker`, up to `terminator`. */
function fieldsBetween(source: string, marker: string, terminator: string): string[] {
  const start = source.indexOf(marker);
  expect(start, `marker not found: ${marker}`).toBeGreaterThan(-1);
  const end = source.indexOf(terminator, start);
  expect(end, `terminator not found after: ${marker}`).toBeGreaterThan(start);
  return [...source.slice(start, end).matchAll(/"([a-zA-Z][a-zA-Z0-9]*)":/g)].map((m) => m[1]);
}

describe('contract: invoice creation response', () => {
  const fields = jsonFields(handler, 'fn invoice_json');

  it('returns the fields the docs tell integrators to read', () => {
    for (const field of ['id', 'status', 'coin', 'amount', 'feeAmount', 'netAmount', 'depositAddress', 'payUrl', 'checkoutUrl', 'qrCode', 'orderId', 'expiresAt', 'createdAt']) {
      expect(fields, `handler must return ${field}`).toContain(field);
      expect(docs, `docs must document ${field}`).toContain(`"${field}"`);
    }
  });

  it('documents no response field the handler does not send', () => {
    // Stop at the closing brace of that documented JSON block.
    const documented = fieldsBetween(docs, '"id": "550e8400-e29b-41d4-a716-446655440000",\n  "status": "PENDING"', '\n}`');
    for (const field of documented) {
      expect(fields, `docs promise ${field}, handler does not send it`).toContain(field);
    }
  });
});

describe('contract: deposit.confirmed webhook', () => {
  const fields = jsonFields(webhook, 'pub fn invoice_payload');

  it('sends exactly the body the docs show', () => {
    for (const field of ['event', 'invoiceId', 'orderId', 'siteUserId', 'coin', 'amount', 'fee', 'netAmount', 'txHash', 'status', 'paidAt', 'customerEmail', 'timestamp', 'attempt']) {
      expect(fields, `webhook must send ${field}`).toContain(field);
      expect(docs, `docs must document ${field}`).toContain(`"${field}"`);
    }
  });

  it('agrees with the docs on the event name and signature format', () => {
    expect(webhook).toContain('pub const EVENT_DEPOSIT_CONFIRMED: &str = "deposit.confirmed"');
    expect(webhook).toContain('format!("sha256={signature}")');
    expect(docs).toContain('deposit.confirmed');
    expect(docs).toContain('X-SatsPay-Signature: sha256=');
  });

  it('uses the same 300s replay window on both sides', () => {
    expect(webhook).toContain('Duration::from_secs(300)');
    expect(docs).toContain('300s');
  });
});

describe('contract: error codes', () => {
  it('every code in the docs table is emitted by the backend', () => {
    const publicApi = readFileSync(path.join(repoRoot, 'crates/api-http/src/public_api.rs'), 'utf8');
    const rateLimit = readFileSync(path.join(repoRoot, 'crates/api-http/src/rate_limit.rs'), 'utf8');
    const backend = handler + publicApi + rateLimit;

    const tableStart = docs.indexOf("['INVALID_API_KEY'");
    expect(tableStart).toBeGreaterThan(-1);
    const table = docs.slice(tableStart, docs.indexOf('].map(', tableStart));
    const codes = [...table.matchAll(/'([A-Z_]{4,})'/g)].map((m) => m[1]);
    expect(codes.length).toBeGreaterThan(15);

    for (const code of codes) {
      expect(backend, `docs list ${code}, backend never returns it`).toContain(`"${code}"`);
    }
  });
});

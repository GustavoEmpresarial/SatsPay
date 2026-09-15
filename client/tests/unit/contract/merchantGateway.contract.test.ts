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
const invoiceModel = readFileSync(path.join(repoRoot, 'crates/db/src/merchant_deposits.rs'), 'utf8');
const dashboard = readFileSync(path.join(clientRoot, 'src/pages/MerchantDepositsPage.tsx'), 'utf8');

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

describe('contract: invoice list / detail casing', () => {
  it('serializes the invoice struct as camelCase', () => {
    // GET /v1/merchant/deposits[/:id] serializes this struct straight to
    // JSON. Without the rename it emitted order_id / fee_amount and the
    // merchant dashboard rendered blank rows.
    const decl = invoiceModel.indexOf('pub struct MerchantDepositInvoice');
    expect(decl).toBeGreaterThan(-1);
    expect(invoiceModel.slice(Math.max(0, decl - 400), decl)).toContain('rename_all = "camelCase"');
  });

  it('gives the dashboard the field names it reads', () => {
    const iface = dashboard.slice(dashboard.indexOf('interface InvoiceItem'), dashboard.indexOf('interface WebhookTestResult'));
    const fields = [...iface.matchAll(/^\s{2}(\w+)\??:/gm)].map((m) => m[1]);
    expect(fields.length).toBeGreaterThan(8);
    // Every field the table reads must exist on the Rust struct (snake_case
    // there, camelCase on the wire).
    for (const field of fields) {
      const snake = field.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
      expect(invoiceModel, `dashboard reads ${field}, struct has no ${snake}`).toContain(`pub ${snake}:`);
    }
  });
});

describe('contract: gateway fee', () => {
  it('publishes the rate the backend actually charges', () => {
    // The fee is a single hardcoded rate; the docs page mirrors it in a
    // constant. If one moves without the other, merchants are quoted a price
    // the ledger does not honour.
    const backend = /pub const GATEWAY_FEE_BPS: u32 = (\d+)/.exec(invoiceModel);
    expect(backend, 'GATEWAY_FEE_BPS must stay findable').toBeTruthy();
    const docsConst = /const GATEWAY_FEE_BPS = (\d+)/.exec(docs);
    expect(docsConst, 'the docs page must mirror the constant').toBeTruthy();
    expect(docsConst![1]).toBe(backend![1]);
    expect(backend![1]).toBe('25');
  });

  it('documents both pricing modes and who carries the price movement', () => {
    // The merchant has to know which field to send, that they are mutually
    // exclusive, and whose money moves while the quote is locked.
    expect(docs).toContain('amountUsd');
    expect(docs).toContain('AMBIGUOUS_AMOUNT');
    expect(docs).toMatch(/amountUsd.{0,80}decimal/is);
    expect(docs).toMatch(/cotação trava|cotação é travada/i);
    expect(docs).toMatch(/arredonda.{0,40}para cima/is);
    expect(docs).toContain('select-coin');
    expect(docs).toContain('COIN_LOCKED');
    expect(docs).toContain('COIN_NOT_ACCEPTED');
    expect(docs).toContain('/v1/merchant/settings');
  });

  it('only documents select-coin errors the handler can actually return', () => {
    const handlerCodes = [...handler.matchAll(/"([A-Z_]{4,})"/g)].map((m) => m[1]);
    for (const code of ['COIN_LOCKED', 'COIN_NOT_ACCEPTED', 'PRICE_UNAVAILABLE', 'AMBIGUOUS_AMOUNT', 'NO_USABLE_COIN', 'INVALID_AMOUNT_USD']) {
      expect(handlerCodes, `docs promise ${code}`).toContain(code);
    }
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

describe('contract: public checkout payload', () => {
  // The checkout is rendered from this payload by CheckoutPage and by the
  // demo. When a field was added on one side only, the customer saw a blank
  // picker on a merchant who had configured several coins.
  /** Field names anywhere inside a function body (nested json! blocks included). */
  function fieldsInFn(source: string, marker: string): string[] {
    const start = source.indexOf(marker);
    expect(start, `function not found: ${marker}`).toBeGreaterThan(-1);
    // Up to the next top-level item, or the end of the file.
    const rest = source.slice(start + marker.length);
    const end = rest.search(/\n(?:pub )?(?:async )?fn |\n#\[cfg\(test\)\]/);
    return [...(end === -1 ? rest : rest.slice(0, end)).matchAll(/"([a-zA-Z][a-zA-Z0-9]*)":/g)].map((m) => m[1]);
  }

  const fields = fieldsInFn(handler, 'fn public_invoice_json');
  const demoFields = fieldsInFn(
    readFileSync(path.join(repoRoot, 'crates/api-http/src/public_catalog.rs'), 'utf8'),
    'fn demo_payload',
  );

  it('sends everything the picker and the QR need', () => {
    for (const field of ['coin', 'amount', 'amountDisplay', 'depositAddress', 'qrCode', 'coinOptions', 'coinLocked', 'amountUsd', 'logoUrl']) {
      expect(fields, `public_invoice_json must send ${field}`).toContain(field);
    }
  });

  it('the demo speaks the same payload as a real invoice', () => {
    // The demo is the one checkout an integrator looks at before signing up,
    // so a field it omits reads as a feature that does not exist.
    for (const field of fields) {
      expect(demoFields, `demo is missing ${field}, so it renders a different checkout`).toContain(field);
    }
  });

  it("never leaks the merchant's own integration fields to the payer", () => {
    // The public payload is served to whoever holds the link.
    for (const secret of ['callbackUrl', 'siteUserId', 'webhookAttempts']) {
      expect(fields, `public payload must not expose ${secret}`).not.toContain(secret);
    }
  });
});

describe('contract: onboarding path', () => {
  const merchant = readFileSync(path.join(repoRoot, 'crates/api-http/src/merchant.rs'), 'utf8');
  const publicApi = readFileSync(path.join(repoRoot, 'crates/api-http/src/public_api.rs'), 'utf8');

  it('documents the apply/status routes the server actually mounts', () => {
    expect(merchant).toContain('"/v1/merchant/apply"');
    expect(merchant).toContain('"/v1/merchant/status"');
    expect(docs).toContain('path="/v1/merchant/apply"');
    expect(docs).toContain('path="/v1/merchant/status"');
  });

  it('names the key-issuing body fields the handler deserializes', () => {
    for (const field of ['label', 'scopes', 'allowedIps', 'expiresInDays', 'requireSignature']) {
      expect(publicApi, `IssueKeyRequest must accept ${field}`).toContain(field);
      expect(docs, `docs must document ${field}`).toContain(field);
    }
  });

  it('tells integrators to read the field the response actually carries', () => {
    // `IssuedKey` has no serde rename, so the secret arrives as `key`.
    // The published guide said `apiKey`, which is simply absent.
    const issued = readFileSync(path.join(repoRoot, 'crates/db/src/public_api.rs'), 'utf8');
    const struct = issued.slice(issued.indexOf('pub struct IssuedKey'));
    expect(struct.slice(0, struct.indexOf('}'))).toContain('pub key: String');
    expect(docs).not.toContain('apiKey');
  });

  it('only advertises scopes the backend enforces', () => {
    const enforced = [...publicApi.matchAll(/require_scope\(&\w+, "(\w+)"\)/g)].map((m) => m[1]);
    expect(enforced).toContain('send');
    // `balance` was advertised for years and is checked nowhere.
    expect(enforced).not.toContain('balance');
    expect(docs).not.toMatch(/scopes.*"balance"/);
  });
});

describe('contract: the HMAC guide matches the verifier', () => {
  const guide = readFileSync(path.join(repoRoot, 'docs/api/public-api-hmac.md'), 'utf8');
  const dbApi = readFileSync(path.join(repoRoot, 'crates/db/src/public_api.rs'), 'utf8');
  const httpApi = readFileSync(path.join(repoRoot, 'crates/api-http/src/public_api.rs'), 'utf8');

  it('names the headers the extractor actually reads', () => {
    for (const header of ['x-key-id', 'x-timestamp', 'x-signature', 'x-api-key']) {
      expect(httpApi, `extractor must read ${header}`).toContain(`"${header}"`);
      expect(guide, `guide must document ${header}`).toContain(header);
    }
  });

  it('documents the canonical string in the order it is built', () => {
    // `canonical_string` is `{timestamp}\n{METHOD}\n{path}\n{body_hash}`.
    expect(dbApi).toContain('format!("{timestamp}\\n{}\\n{path}\\n{body_hash}"');
    expect(guide).toContain('{TIMESTAMP}\\n{METHOD}\\n{PATH}\\n{SHA256_HEX(BODY)}');
  });

  it('does not resurrect the protocol that was never implemented', () => {
    // Replay is stopped by reserving the signature itself, not by a nonce.
    expect(dbApi).toContain('public_api_signature_nonces');
    // The closing section exists to name the old claims and correct them, so
    // check the instructional part of the guide, not the changelog.
    const instructions = guide.split('## 7. O que mudou')[0];
    expect(instructions).not.toContain('X-Nonce');
    expect(instructions).not.toContain('127.0.0.1:4000');
    expect(instructions).not.toContain('transactionId');
    // …and the changelog must actually be there, or the correction is silent.
    expect(guide).toContain('## 7. O que mudou');
  });
});

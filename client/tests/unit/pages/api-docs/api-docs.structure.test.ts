import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/ApiDocsPage.tsx');
const src = readFileSync(pagePath, 'utf8');

describe('API Docs — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/api\"");
    expect(app).toContain("path=\"/docs\"");
    expect(app).toContain("path=\"/api-docs\"");
    expect(app).toContain("ApiDocsPage");
  });

  it('exports a React page component', () => {
    expect(src).toMatch(/export function \w+/);
  });
});

/**
 * The published contract drifted far enough from the API that integrations
 * written from this page could not work at all: a GET-only create path, a
 * `checkoutUrl` field that did not exist, a webhook event and signature
 * format that were never emitted. These assertions pin the corrections.
 */
describe('API Docs — matches the real gateway contract', () => {
  it('documents the create endpoint that actually accepts POST', () => {
    expect(src).toContain('path="/v1/merchant/deposits"');
    expect(src).toContain('curl -X POST ${API_BASE}/v1/merchant/deposits');
    expect(src).toContain('201 Created');
  });

  it('reads checkoutUrl from the response and still mentions payUrl', () => {
    expect(src).toContain('invoice.checkoutUrl');
    expect(src).toContain('"payUrl"');
    // Every language example must consume the field the API returns.
    for (const snippet of ['invoice.checkoutUrl', "invoice[\"checkoutUrl\"]", "$invoice['checkoutUrl']", 'invoice.CheckoutURL', 'invoice["checkoutUrl"]']) {
      expect(src).toContain(snippet);
    }
  });

  it('teaches the ledger-unit rule for amount', () => {
    expect(src).toContain('AMOUNT_NOT_INTEGER');
    expect(src).toContain('toLedgerUnits');
    expect(src).toMatch(/unidades de 1e-8/);
  });

  it('documents the webhook event and signature the server sends', () => {
    expect(src).toContain('deposit.confirmed');
    expect(src).toContain('X-SatsPay-Signature: sha256=');
    expect(src).toContain('/v1/merchant/webhook-signing-secret');
  });

  it('does not resurrect the contract that never existed', () => {
    expect(src).not.toContain('invoice.paid');
    expect(src).not.toMatch(/t=\$\{timestamp\},v1=/);
    expect(src).not.toContain('data.checkoutUrl'); // old shape read off a 405 response
    expect(src).not.toContain('"status": "PAID"');
  });

  it('renders the coin table from the shared constants so it cannot drift', () => {
    expect(src).toContain('isDepositWithdrawPaused');
    expect(src).toContain('DEPOSIT_PAUSED');
    expect(src).toContain('INTERNAL_AMOUNT_DECIMALS');
    // No hand-written coin list that could disagree with `shared/coins.ts`.
    expect(src).not.toMatch(/BTC, LTC, DOGE, BCH, POL, DGB, SOL, USDT, USDC/);
  });

  it('uses one canonical host everywhere', () => {
    const declaration = "const API_BASE = 'https://www.satspay.pro';";
    expect(src).toContain(declaration);
    // Past the declaration, the host may only appear through the constant —
    // the page used to mix satspay.pro and www.satspay.pro across examples.
    const body = src.split(declaration)[1] ?? '';
    expect(body).not.toContain('https://satspay.pro');
    expect(body).not.toContain('https://www.satspay.pro');
  });

  it('lists error codes the API can actually return', () => {
    for (const code of [
      'INVALID_API_KEY',
      'IP_NOT_ALLOWED',
      'KEY_REQUIRES_SIGNATURE',
      'MISSING_SCOPE',
      'DUPLICATE_ORDER_ID',
      'DEPOSIT_PAUSED',
      'RATE_LIMITED',
    ]) {
      expect(src).toContain(code);
    }
    // These were invented for the docs and are not emitted anywhere.
    expect(src).not.toContain('IP_NOT_WHITELISTED');
    expect(src).not.toContain('RATE_LIMIT_EXCEEDED');
    expect(src).not.toContain('INSUFFICIENT_FUNDS');
  });
});

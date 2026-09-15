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

  it('documents the payment button, the demo and the coin catalogue', () => {
    // These existed only as gaps: /demo was a dead redirect, there was no pay
    // button at all, and prices hid behind /v1/swap/prices.
    expect(src).toContain('/sdk/satspay-pay.js');
    expect(src).toContain('data-checkout_url');
    expect(src).toContain('window.SatsPay.renderButtons()');
    expect(src).toContain('/pay/demo');
    expect(src).toContain('/v1/public/coins');
    expect(src).toContain('logoUrl');
    expect(src).toContain('depositsEnabled');
    // The button must never be documented as taking a key in the browser.
    expect(src).not.toMatch(/data-api_?key/i);
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

/**
 * The page documented how to create an invoice without ever saying how to
 * become a merchant or issue a key, and without the endpoints an integrator
 * uses to test the integration. A reader following it from the top hit a 403
 * and had to read the Rust source to find out why.
 */
describe('API Docs — covers the whole integrator path', () => {
  it('opens with the onboarding trail, in the order it happens', () => {
    expect(src).toContain("handleTabChange('start')");
    expect(src).toContain('function OnboardingTab()');
    const tab = src.slice(src.indexOf('function OnboardingTab()'), src.indexOf('export function ApiDocsPage'));
    const order = ['/v1/merchant/apply', '/v1/merchant/status', '/v1/api-keys'];
    let cursor = -1;
    for (const step of order) {
      const at = tab.indexOf(step);
      expect(at, `onboarding must document ${step}`).toBeGreaterThan(-1);
      expect(at, `${step} is out of order`).toBeGreaterThan(cursor);
      cursor = at;
    }
  });

  it('documents key rotation and revocation, not just issuing', () => {
    expect(src).toContain('/v1/api-keys/:id/rotate');
    expect(src).toContain('path="/v1/api-keys/:id"');
    // The secret is unrecoverable; saying so is the whole point.
    expect(src).toMatch(/uma única vez/);
  });

  it('documents how to test an integration before shipping it', () => {
    expect(src).toContain('/v1/merchant/deposits/:id/test-webhook');
    expect(src).toContain('/v1/public/pay/demo');
    expect(src).toContain('/v1/public/pay/demo/select-coin');
    expect(src).toContain('/v1/public/pay/:id/balance');
  });

  it('gives the manual OAuth flow its parameters, not just the SDK path', () => {
    expect(src).toContain('/v1/oauth/authorize/info');
    expect(src).toContain('path="/v1/oauth/authorize"');
    for (const param of ['client_id', 'redirect_uri', 'code_challenge', 'code_challenge_method', 'state']) {
      expect(src, `authorize must document ${param}`).toContain(param);
    }
    expect(src).toContain('S256');
  });

  it('documents managing OAuth apps and revoking consent', () => {
    expect(src).toContain('/v1/oauth/apps');
    expect(src).toContain('/v1/oauth/apps/:id/rotate-secret');
    expect(src).toContain('/v1/oauth/authorized-apps');
  });

  it('never tells anyone to put a key in the browser', () => {
    expect(src).not.toMatch(/data-api_?key/i);
    // The button SDK takes a checkoutUrl the backend already created, and the
    // page has to say so — a reader who guesses wrong exposes the key.
    expect(src).toContain('Nenhuma chave vai para o navegador');
    expect(src).toContain('data-checkout_url');
  });

  it('states the fee once, from the mirrored constant', () => {
    expect(src).toContain('const GATEWAY_FEE_BPS = 25;');
    // No hand-typed percentage that could drift from the constant.
    expect(src.split('const GATEWAY_FEE_PERCENT')[1]).not.toContain('0,5%');
  });
});

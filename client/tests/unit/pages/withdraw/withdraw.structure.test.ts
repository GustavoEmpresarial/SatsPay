import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/WithdrawPage.tsx');

describe('Withdraw — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain("path=\"/withdraw\"");
    expect(app).toContain("WithdrawPage");
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \w+/);
  });

  it('sends email OTP on withdraw, not TOTP', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('emailCode');
    expect(src).toContain('codeSent');
    expect(src).not.toContain('totpCode');
  });

  it('picker lists active coins and shows paused networks as disabled rows', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toContain('depositWithdrawActiveCoins');
    expect(src).toContain('pausedCoins');
    expect(src).toContain('pausedSection');
    expect(src).toContain('aria-disabled="true"');
    expect(src).toContain('pausedBadge');
  });

  it('never picks the merchant wallet as the withdrawal source', () => {
    const src = readFileSync(pagePath, 'utf8');
    // The page used to auto-switch to MERCHANT whenever the personal wallet was
    // empty, so business float left on-chain without the user ever choosing it.
    expect(src).not.toContain('setWalletKind');
    expect(src).not.toMatch(/walletKind\s*[,:]/);
  });

  it('keeps merchant infrastructure entirely out of the personal page', () => {
    const src = readFileSync(pagePath, 'utf8');
    // Merchant caixa belongs to the merchant panel. This page must not read it,
    // render it, or offer a path into it.
    expect(src).not.toContain("kind=MERCHANT");
    expect(src).not.toContain("'MERCHANT'");
    expect(src).not.toContain('/wallet/transfer');
    expect(src).not.toMatch(/merchantBal|merchantMap|merchantQ/);
  });
});

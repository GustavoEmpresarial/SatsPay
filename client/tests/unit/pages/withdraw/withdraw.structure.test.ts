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
});

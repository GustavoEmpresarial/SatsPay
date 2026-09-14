import { describe, expect, it } from 'vitest';
import { coinLogo } from '../../../src/lib/coinAssets.js';
import { COINS } from '../../../src/shared/coins.js';

describe('coinLogo', () => {
  it('maps every live coin to a https URL', () => {
    for (const c of COINS) {
      const url = coinLogo(c);
      expect(url.startsWith('https://')).toBe(true);
    }
  });

  it('SOL uses solana token-list asset', () => {
    expect(coinLogo('SOL')).toContain('solana');
  });

  it('unknown falls back to generic', () => {
    expect(coinLogo('XYZ')).toContain('generic');
  });
});

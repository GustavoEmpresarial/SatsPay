import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { coinLogo, coinLogoAbsolute } from '../../../src/lib/coinAssets.js';
import { COINS } from '../../../src/shared/coins.js';

const publicDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../public');

describe('coinLogo', () => {
  it('serves every live coin from this origin', () => {
    // Icons used to come from a third-party CDN, so the hosted checkout could
    // not render its coin without someone else's uptime.
    for (const c of COINS) {
      expect(coinLogo(c)).toBe(`/sdk/coins/${c.toLowerCase()}.svg`);
    }
  });

  it('ships a real file for every coin it points at', () => {
    for (const c of [...COINS, 'XYZ']) {
      const file = path.join(publicDir, coinLogo(c));
      expect(existsSync(file), `missing icon file for ${c}: ${coinLogo(c)}`).toBe(true);
      expect(readFileSync(file, 'utf8').slice(0, 5)).toMatch(/<svg|<\?xml/);
    }
  });

  it('unknown symbols fall back to the generic icon instead of a 404', () => {
    expect(coinLogo('XYZ')).toBe('/sdk/coins/generic.svg');
    expect(coinLogo('')).toBe('/sdk/coins/generic.svg');
  });

  it('can be made absolute for use outside this origin', () => {
    expect(coinLogoAbsolute('USDT', 'https://www.satspay.pro')).toBe(
      'https://www.satspay.pro/sdk/coins/usdt.svg',
    );
    expect(coinLogoAbsolute('USDT', 'https://www.satspay.pro/')).toBe(
      'https://www.satspay.pro/sdk/coins/usdt.svg',
    );
  });
});

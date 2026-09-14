import { describe, expect, it } from 'vitest';
import { addressQrDataUrl } from '../../../src/lib/qr.js';

describe('addressQrDataUrl', () => {
  it('returns a PNG data URL', async () => {
    const url = await addressQrDataUrl('bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh');
    expect(url.startsWith('data:image/png;base64,')).toBe(true);
    expect(url.length).toBeGreaterThan(100);
  });
});

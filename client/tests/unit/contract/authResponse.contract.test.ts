import { describe, expect, it } from 'vitest';
import {
  assertAuthLoginResponse,
  assertSwapPricesResponse,
} from '../../helpers/contracts.js';

describe('contract: auth login response', () => {
  it('accepts valid login payload', () => {
    const issues = assertAuthLoginResponse({
      user: {
        id: '11111111-1111-1111-1111-111111111111',
        email: 'a@b.co',
        username: 'alice',
        twoFactorEnabled: false,
        merchantStatus: 'NONE',
        createdAt: '2024-01-01T00:00:00.000Z',
      },
      accessToken: 'jwt.here',
    });
    expect(issues).toEqual([]);
  });

  it('rejects missing accessToken and refreshToken in body', () => {
    const issues = assertAuthLoginResponse({
      user: {
        id: '1',
        email: 'a@b.co',
        username: 'alice',
        twoFactorEnabled: false,
        merchantStatus: 'NONE',
        createdAt: 'x',
      },
      refreshToken: 'leaked',
    });
    expect(issues.some((i) => i.path === 'accessToken')).toBe(true);
    expect(issues.some((i) => i.path === 'refreshToken')).toBe(true);
  });
});

describe('contract: swap prices', () => {
  it('validates price map shape', () => {
    expect(
      assertSwapPricesResponse({
        priceDecimals: 8,
        prices: { BTC: '1000000000000', LTC: '100000000' },
      }),
    ).toEqual([]);
    expect(assertSwapPricesResponse({ priceDecimals: 8 })).not.toEqual([]);
  });
});

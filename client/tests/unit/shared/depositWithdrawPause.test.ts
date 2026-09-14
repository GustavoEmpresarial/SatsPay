import { describe, expect, it } from 'vitest';
import {
  COINS,
  DEPOSIT_WITHDRAW_PAUSED_COINS,
  defaultDepositWithdrawCoin,
  isDepositWithdrawPaused,
  isSwapL2Coin,
} from '../../../src/shared/coins.js';

describe('deposit/withdraw pause coverage', () => {
  it('pauses exactly BTC LTC DOGE DGB and keeps them in COINS', () => {
    expect([...DEPOSIT_WITHDRAW_PAUSED_COINS].sort()).toEqual(['BTC', 'DGB', 'DOGE', 'LTC']);
    for (const c of DEPOSIT_WITHDRAW_PAUSED_COINS) {
      expect(COINS.includes(c)).toBe(true);
      expect(isDepositWithdrawPaused(c)).toBe(true);
      expect(isDepositWithdrawPaused(c.toLowerCase())).toBe(true);
    }
  });

  it('leaves L2 / BCH / SOL active for deposit-withdraw', () => {
    for (const c of ['POL', 'USDT', 'USDC', 'BCH', 'SOL'] as const) {
      expect(isDepositWithdrawPaused(c)).toBe(false);
    }
  });

  it('defaultDepositWithdrawCoin skips paused preference', () => {
    expect(defaultDepositWithdrawCoin(null)).not.toBe('BTC');
    expect(isDepositWithdrawPaused(defaultDepositWithdrawCoin('BTC'))).toBe(false);
    expect(defaultDepositWithdrawCoin('POL')).toBe('POL');
    expect(defaultDepositWithdrawCoin('ltc')).not.toBe('LTC');
  });

  it('swap L2 allowlist is independent of deposit pause', () => {
    expect(isSwapL2Coin('POL')).toBe(true);
    expect(isDepositWithdrawPaused('POL')).toBe(false);
    expect(isSwapL2Coin('BTC')).toBe(false);
    expect(isDepositWithdrawPaused('BTC')).toBe(true);
  });
});

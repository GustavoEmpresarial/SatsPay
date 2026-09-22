import { describe, expect, it } from 'vitest';
import {
  COINS,
  DEPOSIT_WITHDRAW_PAUSED_COINS,
  defaultDepositWithdrawCoin,
  depositWithdrawActiveCoins,
  depositWithdrawPausedCoinList,
  isDepositWithdrawPaused,
  isSwapL2Coin,
} from '../../../src/shared/coins.js';

describe('deposit/withdraw pause coverage', () => {
  it('pauses BTC LTC DOGE BCH DGB and keeps them in COINS', () => {
    expect([...DEPOSIT_WITHDRAW_PAUSED_COINS].sort()).toEqual([
      'BCH',
      'BTC',
      'DGB',
      'DOGE',
      'LTC',
    ]);
    for (const c of DEPOSIT_WITHDRAW_PAUSED_COINS) {
      expect(COINS.includes(c)).toBe(true);
      expect(isDepositWithdrawPaused(c)).toBe(true);
      expect(isDepositWithdrawPaused(c.toLowerCase())).toBe(true);
    }
  });

  it('leaves L2 / SOL / ZER active for deposit-withdraw', () => {
    for (const c of ['POL', 'USDT', 'USDC', 'SOL', 'ZER'] as const) {
      expect(isDepositWithdrawPaused(c)).toBe(false);
    }
  });

  it('active picker list excludes paused coins', () => {
    const active = depositWithdrawActiveCoins();
    const paused = depositWithdrawPausedCoinList();
    expect(active.every((c) => !isDepositWithdrawPaused(c))).toBe(true);
    expect(paused.every((c) => isDepositWithdrawPaused(c))).toBe(true);
    expect(active.length + paused.length).toBe(COINS.length);
    expect(active).toEqual(expect.arrayContaining(['POL', 'SOL', 'USDT', 'USDC', 'ZER']));
    expect(paused).toEqual(expect.arrayContaining(['BTC', 'BCH', 'DGB']));
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

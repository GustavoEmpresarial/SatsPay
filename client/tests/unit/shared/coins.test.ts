import { describe, expect, it } from 'vitest';
import {
  COIN_CONFIG,
  INTERNAL_AMOUNT_DECIMALS,
  SWAP_DEFAULT_FEE_BPS,
  formatAmount,
  isCoin,
  parseAmount,
} from '../../../src/shared/coins.js';

describe('internal amount scale', () => {
  it('stores every live coin with the ledger decimal scale', () => {
    expect(INTERNAL_AMOUNT_DECIMALS).toBe(8);
    for (const cfg of Object.values(COIN_CONFIG)) {
      expect(cfg.decimals).toBe(INTERNAL_AMOUNT_DECIMALS);
    }
  });

  it('uses the same default swap fee as crates/db/src/swap.rs', () => {
    expect(SWAP_DEFAULT_FEE_BPS).toBe(25);
  });
});

describe('isCoin', () => {
  it('narrows only configured tickers', () => {
    expect(isCoin('BTC')).toBe(true);
    expect(isCoin('USDC')).toBe(true);
    expect(isCoin('ETH')).toBe(false);
  });
});

describe('parseAmount / formatAmount', () => {
  it('round-trips one whole coin through the 8-decimal ledger unit', () => {
    const smallest = parseAmount('1', 'BTC');
    expect(smallest).toBe(10n ** BigInt(INTERNAL_AMOUNT_DECIMALS));
    expect(formatAmount(smallest, 'BTC')).toBe('1');
  });

  it('parses a fractional amount using the coin decimals', () => {
    expect(parseAmount('0.00001000', 'BTC')).toBe(COIN_CONFIG.BTC.withdrawalFee);
    expect(formatAmount(COIN_CONFIG.BTC.withdrawalFee, 'BTC')).toBe('0.00001');
  });
});

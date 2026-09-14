import type { Coin } from '@/shared';

const BASE = 'https://cdn.jsdelivr.net/gh/atomiclabs/cryptocurrency-icons@1a63530be6e374711a8554f31b17e4cb92c25fa5/svg/color';

const slug: Record<string, string> = {
  BTC: 'btc',
  LTC: 'ltc',
  DOGE: 'doge',
  BCH: 'bch',
  POL: 'matic',
  DGB: 'dgb',
  SOL: 'sol',
  USDT: 'usdt',
  USDC: 'usdc',
};

export function coinLogo(coin: Coin | string): string {
  const sym = String(coin || '').toUpperCase();
  if (sym === 'SOL') {
    return 'https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png';
  }
  return `${BASE}/${slug[sym] || 'generic'}.svg`;
}


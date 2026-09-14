import { describe, expect, it } from 'vitest';
import { getExplorerTxUrl } from '../../../src/lib/chainExplorers.js';

describe('getExplorerTxUrl', () => {
  const hash = 'abc123';

  it('maps UTXO and EVM coins', () => {
    expect(getExplorerTxUrl('BTC', hash)).toContain('mempool.space');
    expect(getExplorerTxUrl('ltc', hash)).toContain('litecoinspace');
    expect(getExplorerTxUrl('DOGE', hash)).toContain('dogechain');
    expect(getExplorerTxUrl('BCH', hash)).toContain('bitcoin-cash');
    expect(getExplorerTxUrl('POL', hash)).toContain('polygonscan');
    expect(getExplorerTxUrl('USDT', hash)).toContain('polygonscan');
    expect(getExplorerTxUrl('USDC', hash)).toContain('polygonscan');
    expect(getExplorerTxUrl('DGB', hash)).toContain('digiexplorer');
    expect(getExplorerTxUrl('SOL', hash)).toContain('solscan');
  });

  it('falls back for unknown coins', () => {
    expect(getExplorerTxUrl('XYZ', hash)).toContain('polygonscan');
  });
});

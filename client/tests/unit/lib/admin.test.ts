import { describe, expect, it } from 'vitest';
import {
  HASH_PREFIX_CHARS,
  HASH_SUFFIX_CHARS,
  RPC_ERROR_PREVIEW_MAX_CHARS,
  aggregateTreasuryByCoin,
  explorerAddressUrl,
  explorerTxUrl,
  formatAdminAmount,
  formatAdminDate,
  formatAdminTime,
  treasuryRowsWithBalance,
  humanizeRpcError,
  parseCoin,
  pickDate,
  shortHash,
  withdrawalStatusLabel,
} from '../../../src/lib/admin.js';
import { INTERNAL_AMOUNT_DECIMALS } from '../../../src/shared/coins.js';

describe('parseCoin', () => {
  it('accepts live coins and rejects unknown tickers', () => {
    expect(parseCoin('BTC')).toBe('BTC');
    expect(parseCoin('POL')).toBe('POL');
    expect(parseCoin('USDT')).toBe('USDT');
    expect(parseCoin('XYZ')).toBeUndefined();
    expect(parseCoin('btc')).toBeUndefined();
  });
});

describe('formatAdminAmount', () => {
  it('formats ledger units using the coin decimals', () => {
    const oneCoin = 10n ** BigInt(INTERNAL_AMOUNT_DECIMALS);
    expect(formatAdminAmount(oneCoin.toString(), 'BTC')).toBe('1 BTC');
    expect(formatAdminAmount('XYZ', 'NOTACOIN')).toBe('XYZ NOTACOIN');
  });
});

describe('shortHash', () => {
  it('keeps short values and truncates with the named prefix/suffix', () => {
    expect(shortHash('abcd')).toBe('abcd');
    const value = '0123456789abcdef0123456789abcdef';
    expect(shortHash(value)).toBe(
      `${value.slice(0, HASH_PREFIX_CHARS)}…${value.slice(-HASH_SUFFIX_CHARS)}`,
    );
  });
});

describe('humanizeRpcError', () => {
  it('maps DGB explorer failures and truncates long raw errors', () => {
    expect(humanizeRpcError('digibyte insight timeout')).toBe('Explorer DGB offline');
    expect(humanizeRpcError(null)).toBeNull();
    const long = 'x'.repeat(RPC_ERROR_PREVIEW_MAX_CHARS + 1);
    const preview = humanizeRpcError(long);
    expect(preview).toBeTruthy();
    expect(preview!.length).toBe(RPC_ERROR_PREVIEW_MAX_CHARS);
    expect(preview!.endsWith('…')).toBe(true);
  });
});

describe('aggregateTreasuryByCoin', () => {
  it('sums deposit addresses per coin and keeps the first RPC error', () => {
    const unit = 10n ** BigInt(INTERNAL_AMOUNT_DECIMALS);
    const rows = aggregateTreasuryByCoin([
      { coin: 'POL', onchain: (unit * 2n).toString(), ledger: unit.toString(), error: null },
      { coin: 'POL', onchain: unit.toString(), ledger: '0', error: 'digibyte insight down' },
      { coin: 'BTC', onchain: '0', ledger: unit.toString(), error: null },
    ]);
    const pol = rows.find((r) => r.coin === 'POL');
    const btc = rows.find((r) => r.coin === 'BTC');
    expect(pol?.onchain).toBe((unit * 3n).toString());
    expect(pol?.ledger).toBe(unit.toString());
    expect(pol?.addressCount).toBe(2);
    expect(pol?.error).toBe('digibyte insight down');
    expect(btc?.onchain).toBe('0');
    expect(btc?.ledger).toBe(unit.toString());
    expect(btc?.addressCount).toBe(1);
  });

  it('returns an empty list when there are no wallets', () => {
    expect(aggregateTreasuryByCoin([])).toEqual([]);
  });

  it('hides coins with zero on-chain and zero ledger', () => {
    const unit = 10n ** BigInt(INTERNAL_AMOUNT_DECIMALS);
    const visible = treasuryRowsWithBalance([
      { coin: 'BTC', onchain: '0', ledger: '0', addressCount: 1, error: null },
      { coin: 'POL', onchain: unit.toString(), ledger: '0', addressCount: 1, error: null },
    ]);
    expect(visible.map((r) => r.coin)).toEqual(['POL']);
  });
});

describe('withdrawalStatusLabel', () => {
  it('labels known statuses and passes unknown through', () => {
    expect(withdrawalStatusLabel('PENDING')).toBe('Aguardando aprovação');
    expect(withdrawalStatusLabel('FAILED')).toBe('Falhou (estornado)');
    expect(withdrawalStatusLabel('CANCELLED')).toBe('Cancelado');
    expect(withdrawalStatusLabel('CANCELED')).toBe('Cancelado');
    expect(withdrawalStatusLabel('REJECTED')).toBe('Cancelado');
    expect(withdrawalStatusLabel('APPROVED')).toBe('Aprovado');
    expect(withdrawalStatusLabel('QUEUED')).toBe('Na fila');
    expect(withdrawalStatusLabel('BROADCASTING')).toBe('Transmitindo');
    expect(withdrawalStatusLabel('BROADCASTED')).toBe('Transmitido');
    expect(withdrawalStatusLabel('CONFIRMED')).toBe('Confirmado');
    expect(withdrawalStatusLabel('UNKNOWN_STATUS')).toBe('UNKNOWN_STATUS');
  });
});

describe('admin explorers + dates', () => {
  it('builds address and tx explorer urls for major chains', () => {
    expect(explorerAddressUrl('BTC', 'addr')).toContain('mempool.space/address/addr');
    expect(explorerAddressUrl('LTC', 'addr')).toContain('litecoinspace.org');
    expect(explorerAddressUrl('DOGE', 'addr')).toContain('dogechain.info');
    expect(explorerAddressUrl('BCH', 'addr')).toContain('bitcoin-cash');
    expect(explorerAddressUrl('DGB', 'addr')).toContain('digiexplorer');
    expect(explorerAddressUrl('SOL', 'addr')).toContain('solscan.io/account');
    expect(explorerAddressUrl('POL', 'addr')).toContain('polygonscan.com/address');

    expect(explorerTxUrl('BTC', 'tx')).toContain('mempool.space/tx/tx');
    expect(explorerTxUrl('LTC', 'tx')).toContain('litecoinspace.org/tx');
    expect(explorerTxUrl('DOGE', 'tx')).toContain('dogechain.info/tx');
    expect(explorerTxUrl('BCH', 'tx')).toContain('transaction');
    expect(explorerTxUrl('DGB', 'tx')).toContain('digiexplorer');
    expect(explorerTxUrl('SOL', 'tx')).toContain('solscan.io/tx');
    expect(explorerTxUrl('USDT', 'tx')).toContain('polygonscan.com/tx');
  });

  it('formats dates/times and picks first valid', () => {
    expect(formatAdminDate(null)).toBe('—');
    expect(formatAdminDate('not-a-date')).toBe('—');
    expect(formatAdminDate(new Date('2024-01-15T12:00:00Z'))).toMatch(/\d/);
    expect(formatAdminTime('')).toBe('—');
    expect(formatAdminTime('bogus')).toBe('—');
    expect(formatAdminTime(new Date('2024-01-15T12:34:56Z'))).toMatch(/\d/);
    expect(pickDate(null, '', 'nope', '2024-06-01T00:00:00.000Z')).toBe('2024-06-01T00:00:00.000Z');
    expect(pickDate(new Date('2024-06-01T00:00:00.000Z'))).toMatch(/2024-06-01/);
    expect(pickDate(null, '')).toBeUndefined();
  });

  it('humanizes network / hot-key RPC errors', () => {
    expect(humanizeRpcError('request timed out')).toBe('RPC sem resposta');
    expect(humanizeRpcError('error sending request to node')).toBe('Falha de rede no nó');
    expect(humanizeRpcError('hot key ausente no vault')).toBe('Seed/chave de saque ausente');
    expect(humanizeRpcError('plain rpc message')).toBe('plain rpc message');
  });

  it('keeps unknown ticker extras after known COINS order', () => {
    const rows = aggregateTreasuryByCoin([
      { coin: 'ZZZ', onchain: '1', ledger: '0', error: null },
      { coin: 'BTC', onchain: '2', ledger: '0', error: null },
    ]);
    expect(rows.map((r) => r.coin)).toEqual(['BTC', 'ZZZ']);
  });
});

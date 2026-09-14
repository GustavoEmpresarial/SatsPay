import { randomUUID } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { COIN_CONFIG } from '../../src/shared/coins.js';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

/** Matches `validate_username` max in crates/domain/src/auth/service.rs */
const USERNAME_MAX_LEN = 24;
/** Healthcheck interval (10s) × retries (8) from deploy/docker/docker-compose.yml */
const HEALTHCHECK_RETRIES_MS = 10_000 * 8;
/** BIP-173 example P2WPKH — fixture address, not a live treasury key. */
const FIXTURE_WITHDRAW_ADDRESS = 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4';
/** 32-byte txid hex (SHA-256), the format used by the UTXO coins in this stack. */
const TXID_HEX_CHARS = 64;
const FIXTURE_ONCHAIN_TX_HASH = '0'.repeat(TXID_HEX_CHARS);

/** Same predicate as `get_dashboard_stats` in crates/db/src/admin.rs */
const ONCHAIN_WITHDRAWAL_COUNT_SQL = 'SELECT COUNT(*)::text AS n FROM withdrawals WHERE tx_hash IS NOT NULL';

describe('admin stats on-chain withdrawal count', () => {
  let db: TestPostgres;

  beforeAll(async () => {
    db = await openTestPostgres();
    assertTestDatabaseUrl(db.url);
  }, HEALTHCHECK_RETRIES_MS);

  afterAll(async () => {
    if (!db) return;
    await db.client.end().catch(() => undefined);
    await db.stop();
  });

  it('ignores FAILED and CANCELED withdrawals that have no tx_hash', async () => {
    const amount = COIN_CONFIG.BTC.minWithdrawal;
    const feeAmount = COIN_CONFIG.BTC.withdrawalFee;
    const email = `stats-${randomUUID()}@bitcosats.test`;
    const username = `stt_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);

    await db.client.query('BEGIN');
    try {
      const before = await db.client.query<{ n: string }>(ONCHAIN_WITHDRAWAL_COUNT_SQL);
      const baseline = BigInt(before.rows[0]?.n ?? '0');

      const user = await db.client.query<{ id: string }>(
        'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id',
        [email, 'test-hash-not-verified', username],
      );
      const userId = user.rows[0]?.id;
      expect(userId).toBeDefined();

      const wallet = await db.client.query<{ id: string }>(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id",
        [userId],
      );
      const walletId = wallet.rows[0]?.id;
      expect(walletId).toBeDefined();

      await db.client.query(
        `INSERT INTO withdrawals (wallet_id, to_address, amount, fee_amount, status)
         VALUES ($1, $2, $3, $4, 'FAILED')`,
        [walletId, FIXTURE_WITHDRAW_ADDRESS, amount.toString(), feeAmount.toString()],
      );
      await db.client.query(
        `INSERT INTO withdrawals (wallet_id, to_address, amount, fee_amount, status)
         VALUES ($1, $2, $3, $4, 'CANCELED')`,
        [walletId, FIXTURE_WITHDRAW_ADDRESS, amount.toString(), feeAmount.toString()],
      );
      await db.client.query(
        `INSERT INTO withdrawals (wallet_id, to_address, amount, fee_amount, status, tx_hash)
         VALUES ($1, $2, $3, $4, 'CONFIRMED', $5)`,
        [walletId, FIXTURE_WITHDRAW_ADDRESS, amount.toString(), feeAmount.toString(), FIXTURE_ONCHAIN_TX_HASH],
      );

      const after = await db.client.query<{ n: string }>(ONCHAIN_WITHDRAWAL_COUNT_SQL);
      expect(BigInt(after.rows[0]?.n ?? '0')).toBe(baseline + 1n);
    } finally {
      await db.client.query('ROLLBACK');
    }

    const leftover = await db.client.query('SELECT id FROM users WHERE email = $1', [email]);
    expect(leftover.rowCount).toBe(0);
  });
});

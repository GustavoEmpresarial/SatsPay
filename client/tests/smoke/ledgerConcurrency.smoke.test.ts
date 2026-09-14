import { randomUUID } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import pg from 'pg';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

/** Healthcheck interval (10s) × retries (8) */
const HEALTHCHECK_RETRIES_MS = 10_000 * 8;
const USERNAME_MAX_LEN = 24;

/**
 * Concurrency (#24): parallel inserts with the same idempotency key —
 * exactly one must win; ledger SUM must stay consistent.
 */
describe('ledger concurrency smoke', () => {
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

  it('parallel duplicate reference_key: only one debit lands', async () => {
    const email = `conc-${randomUUID()}@bitcosats.test`;
    const username = `c_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);
    const refKey = `conc:swap:${randomUUID()}`;

    const user = await db.client.query<{ id: string }>(
      'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id',
      [email, 'hash', username],
    );
    const wallet = await db.client.query<{ id: string }>(
      "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id",
      [user.rows[0]!.id],
    );
    const walletId = wallet.rows[0]!.id;

    await db.client.query(
      `INSERT INTO ledger_entries (wallet_id, amount, type, memo) VALUES ($1, $2, 'DEPOSIT', 'open')`,
      [walletId, '500000000'],
    );

    const insertSql = `
      INSERT INTO ledger_entries (wallet_id, amount, type, reference_key, reference_type, memo)
      VALUES ($1, $2, 'SWAP_OUT', $3, 'Swap', 'race')
    `;

    // Separate connections so queries truly race (pg.Client is not concurrent)
    const racers = await Promise.all(
      Array.from({ length: 4 }, async () => {
        const c = new pg.Client({ connectionString: db.url });
        await c.connect();
        return c;
      }),
    );

    try {
      const results = await Promise.allSettled(
        racers.map((c) => c.query(insertSql, [walletId, '-100000000', refKey])),
      );

      const ok = results.filter((r) => r.status === 'fulfilled').length;
      const rejected = results.filter((r) => r.status === 'rejected').length;
      expect(ok).toBe(1);
      expect(rejected).toBe(3);

      const count = await db.client.query<{ n: string }>(
        `SELECT COUNT(*)::text AS n FROM ledger_entries WHERE reference_key = $1`,
        [refKey],
      );
      expect(Number(count.rows[0]!.n)).toBe(1);

      const bal = await db.client.query<{ balance: string }>(
        `SELECT COALESCE(SUM(amount), 0)::text AS balance FROM ledger_entries WHERE wallet_id = $1`,
        [walletId],
      );
      expect(bal.rows[0]!.balance).toBe('400000000');
    } finally {
      await Promise.all(racers.map((c) => c.end().catch(() => undefined)));
      await db.client.query('DELETE FROM users WHERE id = $1', [user.rows[0]!.id]);
    }
  });
});

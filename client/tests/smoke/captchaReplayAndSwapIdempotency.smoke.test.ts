import { randomUUID } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

const USERNAME_MAX_LEN = 24;
const HEALTHCHECK_RETRIES_MS = 10_000 * 8;

describe('captcha anti-replay + swap idempotency smoke', () => {
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

  it('rejects duplicate captcha_seen_tokens hash (anti-replay)', async () => {
    await db.client.query('BEGIN');
    try {
      const hash = `smoke_${randomUUID().replace(/-/g, '')}`;
      const a = await db.client.query(
        `INSERT INTO captcha_seen_tokens (token_hash) VALUES ($1) ON CONFLICT DO NOTHING RETURNING token_hash`,
        [hash],
      );
      expect(a.rowCount).toBe(1);
      const b = await db.client.query(
        `INSERT INTO captcha_seen_tokens (token_hash) VALUES ($1) ON CONFLICT DO NOTHING RETURNING token_hash`,
        [hash],
      );
      expect(b.rowCount).toBe(0);
      await db.client.query('ROLLBACK');
    } catch (e) {
      await db.client.query('ROLLBACK').catch(() => undefined);
      throw e;
    }
  });

  it('swap idempotency key prevents double debit at ledger level', async () => {
    await db.client.query('BEGIN');
    try {
      const email = `swap-idemp-${randomUUID()}@bitcosats.test`;
      const username = `sw_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);
      const user = await db.client.query<{ id: string }>(
        'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id',
        [email, 'test-hash', username],
      );
      const userId = user.rows[0]!.id;
      const wallet = await db.client.query<{ id: string }>(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id",
        [userId],
      );
      const walletId = wallet.rows[0]!.id;
      const refKey = `swap:smoke:${randomUUID()}`;

      await db.client.query(
        `INSERT INTO ledger_entries (wallet_id, amount, type, reference_key, reference_type, memo)
         VALUES ($1, $2, 'SWAP_OUT', $3, 'Swap', 'first')`,
        [walletId, '-1000000', refKey],
      );

      const before = await db.client.query<{ n: string }>(
        `SELECT COUNT(*)::text AS n FROM ledger_entries WHERE reference_key = $1`,
        [refKey],
      );
      expect(Number(before.rows[0]!.n)).toBe(1);

      await db.client.query('SAVEPOINT before_replay');
      let replayBlocked = false;
      try {
        await db.client.query(
          `INSERT INTO ledger_entries (wallet_id, amount, type, reference_key, reference_type, memo)
           VALUES ($1, $2, 'SWAP_OUT', $3, 'Swap', 'replay')`,
          [walletId, '-1000000', refKey],
        );
      } catch {
        replayBlocked = true;
        await db.client.query('ROLLBACK TO SAVEPOINT before_replay');
      }
      expect(replayBlocked).toBe(true);

      const count = await db.client.query<{ n: string }>(
        `SELECT COUNT(*)::text AS n FROM ledger_entries WHERE reference_key = $1`,
        [refKey],
      );
      expect(Number(count.rows[0]!.n)).toBe(1);

      await db.client.query('ROLLBACK');
    } catch (e) {
      await db.client.query('ROLLBACK').catch(() => undefined);
      throw e;
    }
  });
});

import { randomUUID } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

/** Healthcheck interval (10s) × retries (8) from deploy/docker/docker-compose.yml */
const HEALTHCHECK_RETRIES_MS = 10_000 * 8;
const USERNAME_MAX_LEN = 24;

describe('auth users + ledger invariants smoke (docker postgres)', () => {
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

  it('enforces unique email and derives wallet balance from ledger SUM only', async () => {
    const email = `auth-${randomUUID()}@bitcosats.test`;
    const username = `usr_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);

    // Unique-email check in its own tx — PG aborts the whole tx after a constraint error
    await db.client.query('BEGIN');
    try {
      await db.client.query(
        'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3)',
        [email, 'argon2-not-verified-in-smoke', username],
      );
      await expect(
        db.client.query(
          'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3)',
          [email, 'other-hash', `${username}x`.slice(0, USERNAME_MAX_LEN)],
        ),
      ).rejects.toThrow(/unique|duplicate/i);
    } finally {
      await db.client.query('ROLLBACK');
    }

    await db.client.query('BEGIN');
    try {
      const user = await db.client.query<{ id: string }>(
        'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id',
        [email, 'argon2-not-verified-in-smoke', username],
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
        `INSERT INTO ledger_entries (wallet_id, amount, type, memo)
         VALUES ($1, $2, 'DEPOSIT', 'opening')`,
        [walletId, '100000000'],
      );
      await db.client.query(
        `INSERT INTO ledger_entries (wallet_id, amount, type, memo)
         VALUES ($1, $2, 'WITHDRAWAL', 'partial out')`,
        [walletId, '-25000000'],
      );
      await db.client.query(
        `INSERT INTO ledger_entries (wallet_id, amount, type, memo)
         VALUES ($1, $2, 'SWAP_FEE', 'fee')`,
        [walletId, '-100'],
      );

      const bal = await db.client.query<{ balance: string }>(
        `SELECT COALESCE(SUM(amount), 0)::text AS balance
         FROM ledger_entries WHERE wallet_id = $1`,
        [walletId],
      );
      expect(bal.rows[0]?.balance).toBe('74999900');

      // Golden rule: wallets table must not store a mutable balance column as source of truth
      const cols = await db.client.query<{ column_name: string }>(
        `SELECT column_name FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = 'wallets'`,
      );
      const names = cols.rows.map((r) => r.column_name);
      expect(names).not.toContain('balance');
    } finally {
      await db.client.query('ROLLBACK');
    }
  });

  it('rejects double-credit with same reference_id + type (dedup index)', async () => {
    const email = `dedup-${randomUUID()}@bitcosats.test`;
    const username = `dd_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);
    const refId = randomUUID();

    await db.client.query('BEGIN');
    try {
      const user = await db.client.query<{ id: string }>(
        'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id',
        [email, 'hash', username],
      );
      const wallet = await db.client.query<{ id: string }>(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id",
        [user.rows[0]!.id],
      );
      const walletId = wallet.rows[0]!.id;

      await db.client.query(
        `INSERT INTO ledger_entries (wallet_id, amount, type, reference_id, reference_type, memo)
         VALUES ($1, $2, 'DEPOSIT', $3, 'Deposit', 'first')`,
        [walletId, '50000000', refId],
      );

      await expect(
        db.client.query(
          `INSERT INTO ledger_entries (wallet_id, amount, type, reference_id, reference_type, memo)
           VALUES ($1, $2, 'DEPOSIT', $3, 'Deposit', 'replay')`,
          [walletId, '50000000', refId],
        ),
      ).rejects.toThrow(/unique|duplicate/i);
    } finally {
      await db.client.query('ROLLBACK');
    }
  });
});

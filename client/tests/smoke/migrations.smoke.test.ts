import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

const HEALTHCHECK_RETRIES_MS = 10_000 * 8;
const MIGRATIONS_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../crates/db/migrations',
);

/** Migration / DB schema smoke (#11, #31). */
describe('migrations smoke', () => {
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

  it('applies all sql migrations and exposes core tables', async () => {
    const files = (await readdir(MIGRATIONS_DIR)).filter((f) => f.endsWith('.sql')).sort();
    expect(files.length).toBeGreaterThanOrEqual(20);
    expect(files[0]).toMatch(/^0001_/);

    const tables = await db.client.query<{ table_name: string }>(
      `SELECT table_name FROM information_schema.tables
       WHERE table_schema = 'public' AND table_type = 'BASE TABLE'`,
    );
    const names = new Set(tables.rows.map((r) => r.table_name));
    for (const required of [
      'users',
      'wallets',
      'ledger_entries',
      'deposits',
      'withdrawals',
      'outbox_events',
    ]) {
      expect(names.has(required), `missing table ${required}`).toBe(true);
    }
  });

  it('ledger golden rule: no balance column on wallets', async () => {
    const cols = await db.client.query<{ column_name: string }>(
      `SELECT column_name FROM information_schema.columns
       WHERE table_schema = 'public' AND table_name = 'wallets'`,
    );
    expect(cols.rows.map((r) => r.column_name)).not.toContain('balance');
  });

  it('foreign keys: deleting user cascades wallets', async () => {
    const email = `mig-fk-${Date.now()}@bitcosats.test`;
    const user = await db.client.query<{ id: string }>(
      `INSERT INTO users (email, password_hash, username)
       VALUES ($1, 'h', $2) RETURNING id`,
      [email, `mig_${Date.now()}`.slice(0, 24)],
    );
    const userId = user.rows[0]!.id;
    await db.client.query(
      `INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL')`,
      [userId],
    );
    await db.client.query(`DELETE FROM users WHERE id = $1`, [userId]);
    const left = await db.client.query(
      `SELECT 1 FROM wallets WHERE user_id = $1`,
      [userId],
    );
    expect(left.rowCount).toBe(0);
  });
});

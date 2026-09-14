import { randomUUID } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

/** Matches `validate_username` max in crates/domain/src/auth/service.rs */
const USERNAME_MAX_LEN = 24;
/** Healthcheck interval (10s) × retries (8) from deploy/docker/docker-compose.yml */
const HEALTHCHECK_RETRIES_MS = 10_000 * 8;

describe('user audit + system error collection', () => {
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

  it('stores a user-visible audit row and a client-frontend error fingerprint', async () => {
    const email = `logs-${randomUUID()}@bitcosats.test`;
    const username = `log_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);

    await db.client.query('BEGIN');
    try {
      const user = await db.client.query<{ id: string }>(
        'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id',
        [email, 'test-hash-not-verified', username],
      );
      const userId = user.rows[0]?.id;
      expect(userId).toBeDefined();

      await db.client.query(
        `INSERT INTO audit_logs (user_id, action, entity, entity_id, ip, metadata)
         VALUES ($1, 'AUTH_LOGIN_FAILED', 'User', $1, $2, $3::jsonb)`,
        [userId, '127.0.0.1', JSON.stringify({ attemptedEmail: email })],
      );

      const logs = await db.client.query<{ action: string }>(
        'SELECT action FROM audit_logs WHERE user_id = $1',
        [userId],
      );
      expect(logs.rows.map((r) => r.action)).toContain('AUTH_LOGIN_FAILED');

      await db.client.query(
        `INSERT INTO system_error_logs (
           fingerprint, service, level, message, endpoint, status_code, user_id, status
         ) VALUES ($1, 'client-frontend', 'ERROR', $2, $3, $4, $5, 'OPEN')`,
        [`fp-${randomUUID()}`, '500 /wallet: boom', '/wallet', 500, userId],
      );

      const errors = await db.client.query<{ service: string; user_id: string }>(
        `SELECT service, user_id::text AS user_id FROM system_error_logs
         WHERE user_id = $1 AND service = 'client-frontend'`,
        [userId],
      );
      expect(errors.rowCount).toBe(1);
      expect(errors.rows[0]?.service).toBe('client-frontend');
    } finally {
      await db.client.query('ROLLBACK');
    }
  });
});

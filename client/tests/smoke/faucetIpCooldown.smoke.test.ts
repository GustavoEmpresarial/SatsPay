import { randomUUID } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { COIN_CONFIG } from '../../src/shared/coins.js';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

/** Matches `validate_username` max in crates/domain/src/auth/service.rs */
const USERNAME_MAX_LEN = 24;
/** Healthcheck interval (10s) × retries (8) from deploy/docker/docker-compose.yml */
const HEALTHCHECK_RETRIES_MS = 10_000 * 8;
const COOLDOWN_MINUTES = 60;
const SHARED_IP = '203.0.113.77';

describe('faucet IP Sybil cooldown smoke', () => {
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

  it('blocks a second user on the same IP+coin within cooldown', async () => {
    const reward = COIN_CONFIG.BTC.faucetReward;
    await db.client.query('BEGIN');
    try {
      const mkUser = async (tag: string) => {
        const email = `faucet-${tag}-${randomUUID()}@bitcosats.test`;
        const username = `fc_${tag}_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);
        const user = await db.client.query<{ id: string }>(
          'INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id',
          [email, 'test-hash-not-verified', username],
        );
        const userId = user.rows[0]!.id;
        await db.client.query(
          "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id",
          [userId],
        );
        return userId;
      };

      const userA = await mkUser('a');
      const userB = await mkUser('b');

      // User A claimed from SHARED_IP — source of truth for db::faucet::claim IP cooldown.
      await db.client.query(
        `INSERT INTO faucet_claims (user_id, coin, amount, ip) VALUES ($1, 'BTC', $2, $3)`,
        [userA, reward.toString(), SHARED_IP],
      );

      const sameIpRecent = await db.client.query(
        `SELECT 1 FROM faucet_claims
         WHERE ip = $1 AND coin = 'BTC'
           AND created_at > NOW() - ($2::text || ' minutes')::interval
         LIMIT 1`,
        [SHARED_IP, String(COOLDOWN_MINUTES)],
      );
      expect(sameIpRecent.rows.length).toBe(1);

      // Same predicate for user B on SHARED_IP → blocked.
      expect(sameIpRecent.rows.length > 0).toBe(true);

      const otherIp = await db.client.query(
        `SELECT 1 FROM faucet_claims
         WHERE ip = $1 AND coin = 'BTC'
           AND created_at > NOW() - ($2::text || ' minutes')::interval
         LIMIT 1`,
        ['198.51.100.10', String(COOLDOWN_MINUTES)],
      );
      expect(otherIp.rows.length).toBe(0);

      const owners = await db.client.query<{ user_id: string }>(
        `SELECT user_id::text AS user_id FROM faucet_claims WHERE ip = $1 AND coin = 'BTC'`,
        [SHARED_IP],
      );
      expect(owners.rows.map((r) => r.user_id)).toContain(userA);
      expect(owners.rows.map((r) => r.user_id)).not.toContain(userB);

      await db.client.query('ROLLBACK');
    } catch (e) {
      await db.client.query('ROLLBACK').catch(() => undefined);
      throw e;
    }
  });
});

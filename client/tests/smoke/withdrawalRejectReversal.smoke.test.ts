import { randomUUID } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { COIN_CONFIG } from '../../src/shared/coins.js';
import { assertTestDatabaseUrl } from '../helpers/assertTestDatabase.js';
import { openTestPostgres, type TestPostgres } from '../helpers/testPostgres.js';

/** Matches `validate_username` max in crates/domain/src/auth/service.rs */
const USERNAME_MAX_LEN = 24;
/** Same clamp as `list_all_withdrawals` in crates/db/src/admin.rs */
const LIST_LIMIT = 200;
/** Healthcheck interval (10s) × retries (8) from deploy/docker/docker-compose.yml */
const HEALTHCHECK_RETRIES_MS = 10_000 * 8;
/** BIP-173 example P2WPKH — fixture address, not a live treasury key. */
const FIXTURE_WITHDRAW_ADDRESS = 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4';

const LIST_WITHDRAWALS_SQL = `
  SELECT w.id, u.id as user_id, u.email, wa.coin::text as coin, w.to_address, w.amount, w.fee_amount,
         w.status::text as status, w.tx_hash, w.requires_approval, w.created_at
  FROM withdrawals w
  JOIN wallets wa ON wa.id = w.wallet_id
  JOIN users u ON u.id = wa.user_id
  WHERE w.status = $1::withdrawal_status
  ORDER BY w.created_at DESC LIMIT $2
`;

describe('withdrawal reject reversal smoke', () => {
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

  it('credits amount + fee_amount on reject and leaves a clean ledger after rollback', async () => {
    const amount = COIN_CONFIG.BTC.minWithdrawal;
    const feeAmount = COIN_CONFIG.BTC.withdrawalFee;
    const openingCredit = amount + feeAmount;
    const email = `smoke-${randomUUID()}@bitcosats.test`;
    const username = `smk_${randomUUID().replace(/-/g, '')}`.slice(0, USERNAME_MAX_LEN);

    await db.client.query('BEGIN');
    try {
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
        `INSERT INTO ledger_entries (wallet_id, amount, type, memo)
         VALUES ($1, $2, 'DEPOSIT', 'smoke opening credit')`,
        [walletId, openingCredit.toString()],
      );

      const withdrawal = await db.client.query<{ id: string }>(
        `INSERT INTO withdrawals (wallet_id, to_address, amount, fee_amount, status)
         VALUES ($1, $2, $3, $4, 'PENDING') RETURNING id`,
        [walletId, FIXTURE_WITHDRAW_ADDRESS, amount.toString(), feeAmount.toString()],
      );
      const withdrawalId = withdrawal.rows[0]?.id;
      expect(withdrawalId).toBeDefined();

      await db.client.query(
        `INSERT INTO ledger_entries (wallet_id, amount, type, reference_id, reference_type, memo)
         VALUES ($1, $2, 'WITHDRAWAL', $3, 'Withdrawal', 'smoke debit amount')`,
        [walletId, (-amount).toString(), withdrawalId],
      );
      await db.client.query(
        `INSERT INTO ledger_entries (wallet_id, amount, type, reference_id, reference_type, memo)
         VALUES ($1, $2, 'WITHDRAWAL_FEE', $3, 'Withdrawal', 'smoke debit fee')`,
        [walletId, (-feeAmount).toString(), withdrawalId],
      );

      const afterDebit = await db.client.query<{ sum: string }>(
        'SELECT COALESCE(SUM(amount), 0)::text AS sum FROM ledger_entries WHERE wallet_id = $1',
        [walletId],
      );
      expect(afterDebit.rows[0]?.sum).toBe('0');

      const feeCol = await db.client.query<{ column_name: string }>(
        `SELECT column_name FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = 'withdrawals' AND column_name = 'fee_amount'`,
      );
      expect(feeCol.rowCount).toBe(1);
      const legacyFee = await db.client.query(
        `SELECT column_name FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = 'withdrawals' AND column_name = 'fee'`,
      );
      expect(legacyFee.rowCount).toBe(0);

      const loaded = await db.client.query<{ amount: string; fee_amount: string; wallet_id: string }>(
        'SELECT wallet_id, amount::text, fee_amount::text FROM withdrawals WHERE id = $1',
        [withdrawalId],
      );
      expect(loaded.rows[0]?.fee_amount).toBe(feeAmount.toString());
      const reversal = BigInt(loaded.rows[0]!.amount) + BigInt(loaded.rows[0]!.fee_amount);

      const canceled = await db.client.query(
        `UPDATE withdrawals SET status = 'CANCELED'::withdrawal_status, updated_at = now()
         WHERE id = $1 AND status = 'PENDING'::withdrawal_status`,
        [withdrawalId],
      );
      expect(canceled.rowCount).toBe(1);

      await db.client.query(
        `INSERT INTO ledger_entries (wallet_id, amount, type, reference_id, reference_type, memo)
         VALUES ($1, $2, 'WITHDRAWAL_REVERSAL', $3, 'Withdrawal', 'Rejected by admin')`,
        [walletId, reversal.toString(), withdrawalId],
      );

      const afterReject = await db.client.query<{ sum: string }>(
        'SELECT COALESCE(SUM(amount), 0)::text AS sum FROM ledger_entries WHERE wallet_id = $1',
        [walletId],
      );
      expect(afterReject.rows[0]?.sum).toBe(openingCredit.toString());

      const listed = await db.client.query<{
        id: string;
        fee_amount: string;
        status: string;
      }>(LIST_WITHDRAWALS_SQL, ['CANCELED', LIST_LIMIT]);
      const row = listed.rows.find((r) => r.id === withdrawalId);
      expect(row).toBeDefined();
      expect(row?.fee_amount.toString()).toBe(feeAmount.toString());
      expect(row?.status).toBe('CANCELED');
    } finally {
      await db.client.query('ROLLBACK');
    }

    const leftover = await db.client.query('SELECT id FROM users WHERE email = $1', [email]);
    expect(leftover.rowCount).toBe(0);
  });
});

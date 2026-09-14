//! Stake claim + lend supply/borrow/repay/withdraw via `#[sqlx::test]`.

mod common;

use bigdecimal::BigDecimal;
use shared::Coin;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

const TEST_PRICE_MAX_STALE: Duration = Duration::from_secs(86_400);

async fn seed_lend_pool(pool: &PgPool, coins: &[Coin], amount: u64) {
    for &coin in coins {
        for kind in ["HOUSE", "LEND_POOL"] {
            let mut tx = pool.begin().await.unwrap();
            let wallet_id: Uuid = sqlx::query_scalar(
                "SELECT w.id FROM wallets w JOIN users u ON u.id = w.user_id \
                 WHERE u.email = $1 AND w.coin = $2::coin AND w.kind = $3::wallet_kind",
            )
            .bind(db::house::HOUSE_EMAIL)
            .bind(coin.as_str())
            .bind(kind)
            .fetch_one(&mut *tx)
            .await
            .expect("system wallet");
            db::ledger::apply_ledger_entry(
                &mut tx,
                db::ledger::LedgerCreditInput {
                    reference_key: None,
                    wallet_id,
                    amount: BigDecimal::from(amount),
                    ledger_type: "ADJUSTMENT",
                    reference_id: None,
                    reference_type: Some("SqlxTestSeed"),
                    memo: Some("seed"),
                },
            )
            .await
            .expect("seed credit");
            tx.commit().await.unwrap();
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn stake_and_lend_round_trip(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    db::lend::ensure_lend_reserves(&pool).await.expect("lend reserves");
    common::seed_price_cache(&pool).await;
    seed_lend_pool(&pool, &[Coin::Btc, Coin::Ltc], 10_000_000_000_000).await;

    let user_id = common::insert_user(&pool, "stakelend").await;
    let btc_wallet = common::insert_personal_wallet(&pool, user_id, Coin::Btc).await;
    let _ltc_wallet = common::insert_personal_wallet(&pool, user_id, Coin::Ltc).await;
    common::credit_wallet(&pool, btc_wallet, 100_000_000, "seed user").await;

    let stake = db::stake::create_stake(&pool, user_id, Coin::Btc, 10_000_000, 0)
        .await
        .expect("create_stake");
    assert_eq!(stake.status, "ACTIVE");
    assert_eq!(
        common::wallet_balance(&pool, btc_wallet).await,
        BigDecimal::from(90_000_000u64)
    );

    let claimed = db::stake::claim_stake(&pool, user_id, stake.id)
        .await
        .expect("claim_stake");
    assert_eq!(claimed.status, "COMPLETED");
    assert!(db::stake::claim_stake(&pool, user_id, stake.id).await.is_err());

    let supplied = db::lend::supply(&pool, user_id, Coin::Btc, 50_000_000)
        .await
        .expect("supply");
    assert!(supplied > 0);

    let borrowed = db::lend::borrow(&pool, user_id, Coin::Ltc, 1_000_000, TEST_PRICE_MAX_STALE)
        .await
        .expect("borrow");
    assert!(borrowed > 0);

    assert!(
        db::lend::borrow(&pool, user_id, Coin::Ltc, 100_000_000_000, TEST_PRICE_MAX_STALE)
            .await
            .is_err(),
        "over-borrow"
    );

    let repaid = db::lend::repay(&pool, user_id, Coin::Ltc, 1_000_000)
        .await
        .expect("repay");
    assert!(repaid > 0);

    let withdrawn = db::lend::withdraw(&pool, user_id, Coin::Btc, 50_000_000, TEST_PRICE_MAX_STALE)
        .await
        .expect("withdraw");
    assert!(withdrawn > 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn stake_list_strategies_and_cancel(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let strategies = db::stake::list_staking_strategies(&pool).await.unwrap();
    assert!(!strategies.is_empty());

    let user_id = common::insert_user(&pool, "stake-cancel").await;
    let btc = common::insert_personal_wallet(&pool, user_id, Coin::Btc).await;
    common::credit_wallet(&pool, btc, 50_000_000, "seed").await;

    let stake = db::stake::create_stake(&pool, user_id, Coin::Btc, 5_000_000, 0)
        .await
        .unwrap();
    let listed = db::stake::list_stakes(&pool, user_id).await.unwrap();
    assert!(listed.iter().any(|s| s.id == stake.id));

    db::stake::cancel_stake(&pool, user_id, stake.id).await.unwrap();
    assert!(db::stake::cancel_stake(&pool, user_id, stake.id).await.is_err());
    assert_eq!(
        common::wallet_balance(&pool, btc).await,
        BigDecimal::from(50_000_000u64)
    );
}

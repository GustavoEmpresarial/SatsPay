//! Lend liquidation: unhealthy seize + reject self/healthy.

mod common;

use bigdecimal::{BigDecimal, Zero};
use shared::Coin;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

const CLOSE_FACTOR_BPS: u32 = 5000;
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
async fn liquidation_unhealthy_seize_and_guards(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    db::lend::ensure_lend_reserves(&pool).await.expect("lend reserves");
    common::seed_price_cache(&pool).await;
    seed_lend_pool(&pool, &[Coin::Btc, Coin::Ltc], 10_000_000_000_000).await;

    let borrower_id = common::insert_user(&pool, "liq-borrower").await;
    let borrower_btc = common::insert_personal_wallet(&pool, borrower_id, Coin::Btc).await;
    let _borrower_ltc = common::insert_personal_wallet(&pool, borrower_id, Coin::Ltc).await;
    common::credit_wallet(&pool, borrower_btc, 100_000_000, "seed borrower").await;

    db::lend::supply(&pool, borrower_id, Coin::Btc, 100_000_000)
        .await
        .expect("supply");
    db::lend::borrow(&pool, borrower_id, Coin::Ltc, 4_000_000_000, TEST_PRICE_MAX_STALE)
        .await
        .expect("borrow");

    let before = db::lend::get_user_positions(&pool, borrower_id, TEST_PRICE_MAX_STALE)
        .await
        .unwrap();
    assert!(before.health_factor_bps.unwrap() >= 10_000);

    sqlx::query(
        "UPDATE lend_positions SET scaled_supply = scaled_supply / 20 \
         WHERE user_id = $1 AND coin = 'BTC'::coin",
    )
    .bind(borrower_id)
    .execute(&pool)
    .await
    .unwrap();

    let after = db::lend::get_user_positions(&pool, borrower_id, TEST_PRICE_MAX_STALE)
        .await
        .unwrap();
    assert!(after.health_factor_bps.unwrap() < 10_000);

    let liquidatable = db::lend::list_liquidatable_positions(&pool, TEST_PRICE_MAX_STALE)
        .await
        .unwrap();
    assert!(liquidatable.iter().any(|p| p.user_id == borrower_id));

    let liquidator_id = common::insert_user(&pool, "liq-liquidator").await;
    let liquidator_ltc = common::insert_personal_wallet(&pool, liquidator_id, Coin::Ltc).await;
    let liquidator_btc = common::insert_personal_wallet(&pool, liquidator_id, Coin::Btc).await;
    common::credit_wallet(&pool, liquidator_ltc, 10_000_000_000, "seed liquidator").await;

    assert!(
        db::lend::liquidate(
            &pool,
            borrower_id,
            borrower_id,
            Coin::Ltc,
            Coin::Btc,
            1_000_000,
            CLOSE_FACTOR_BPS,
            TEST_PRICE_MAX_STALE,
        )
        .await
        .is_err()
    );

    let result = db::lend::liquidate(
        &pool,
        liquidator_id,
        borrower_id,
        Coin::Ltc,
        Coin::Btc,
        2_000_000_000,
        CLOSE_FACTOR_BPS,
        TEST_PRICE_MAX_STALE,
    )
    .await
    .expect("liquidate");
    assert!(result.repaid > 0 && result.seized > 0);
    assert!(!common::wallet_balance(&pool, liquidator_btc).await.is_zero());

    let healthy_id = common::insert_user(&pool, "liq-healthy").await;
    let healthy_btc = common::insert_personal_wallet(&pool, healthy_id, Coin::Btc).await;
    common::credit_wallet(&pool, healthy_btc, 50_000_000, "seed healthy").await;
    db::lend::supply(&pool, healthy_id, Coin::Btc, 50_000_000)
        .await
        .unwrap();
    assert!(
        db::lend::liquidate(
            &pool,
            liquidator_id,
            healthy_id,
            Coin::Ltc,
            Coin::Btc,
            1_000,
            CLOSE_FACTOR_BPS,
            TEST_PRICE_MAX_STALE,
        )
        .await
        .is_err()
    );
}

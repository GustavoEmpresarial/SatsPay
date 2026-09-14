//! Swap idempotency + faucet cooldown / Sybil IP via `#[sqlx::test]`.

mod common;

use bigdecimal::BigDecimal;
use shared::Coin;
use sqlx::PgPool;
use std::time::Duration;

const TEST_PRICE_MAX_STALE: Duration = Duration::from_secs(86_400);

#[sqlx::test(migrations = "./migrations")]
async fn swap_idempotent_and_faucet_ip_cooldown(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    common::seed_price_cache(&pool).await;
    common::seed_house_liquidity(&pool, &[Coin::Btc, Coin::Ltc], 10_000_000_000_000).await;

    let user_id = common::insert_user(&pool, "swap").await;
    let btc_wallet = common::insert_personal_wallet(&pool, user_id, Coin::Btc).await;
    let _ltc_wallet = common::insert_personal_wallet(&pool, user_id, Coin::Ltc).await;
    common::credit_wallet(&pool, btc_wallet, 100_000_000, "seed user").await;

    let (swap, created) = db::swap::execute(
        &pool,
        user_id,
        Coin::Btc,
        Coin::Ltc,
        10_000_000,
        None,
        "sqlx-swap-1",
        TEST_PRICE_MAX_STALE,
    )
    .await
    .expect("swap");
    assert!(created);
    assert!(swap.to_amount > 0);

    let (swap2, created2) = db::swap::execute(
        &pool,
        user_id,
        Coin::Btc,
        Coin::Ltc,
        10_000_000,
        None,
        "sqlx-swap-1",
        TEST_PRICE_MAX_STALE,
    )
    .await
    .expect("swap replay");
    assert!(!created2);
    assert_eq!(swap.id, swap2.id);

    let btc_balance = common::wallet_balance(&pool, btc_wallet).await;
    assert_eq!(btc_balance, BigDecimal::from(90_000_000u64));

    let claim = db::faucet::claim(&pool, user_id, Coin::Btc, "203.0.113.5", 60)
        .await
        .expect("faucet claim");
    assert!(claim.amount > 0);

    assert!(
        db::faucet::claim(&pool, user_id, Coin::Btc, "203.0.113.5", 60)
            .await
            .is_err(),
        "cooldown"
    );

    let user_b = common::insert_user(&pool, "sybil").await;
    let _btc_b = common::insert_personal_wallet(&pool, user_b, Coin::Btc).await;
    assert!(
        db::faucet::claim(&pool, user_b, Coin::Btc, "203.0.113.5", 60)
            .await
            .is_err(),
        "sybil same IP"
    );

    let other_ip = db::faucet::claim(&pool, user_b, Coin::Btc, "198.51.100.10", 60)
        .await
        .expect("other IP");
    assert!(other_ip.amount > 0);
}

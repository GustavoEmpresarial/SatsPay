//! Reward program distribute by weight + no double-pay on immediate re-tick.

mod common;

use bigdecimal::Zero;
use shared::Coin;
use sqlx::PgPool;
use std::time::Duration;

const TEST_PRICE_MAX_STALE: Duration = Duration::from_secs(86_400);

#[sqlx::test(migrations = "./migrations")]
async fn rewards_weight_distribute_and_deactivate(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    common::seed_price_cache(&pool).await;
    common::seed_house_liquidity(&pool, &[Coin::Ltc], 1_000_000_000).await;

    let admin_id = common::insert_user(&pool, "rewards-admin").await;
    let user_a = common::insert_user(&pool, "rewards-a").await;
    let user_b = common::insert_user(&pool, "rewards-b").await;

    let user_a_btc = common::insert_personal_wallet(&pool, user_a, Coin::Btc).await;
    let user_a_ltc = common::insert_personal_wallet(&pool, user_a, Coin::Ltc).await;
    let user_b_btc = common::insert_personal_wallet(&pool, user_b, Coin::Btc).await;
    let user_b_ltc = common::insert_personal_wallet(&pool, user_b, Coin::Ltc).await;

    common::credit_wallet(&pool, user_a_btc, 30_000_000, "weight a").await;
    common::credit_wallet(&pool, user_b_btc, 10_000_000, "weight b").await;

    let start_at = chrono::Utc::now() - chrono::Duration::hours(1);
    let program = db::rewards::create_program(
        &pool,
        admin_id,
        Coin::Ltc,
        Coin::Btc,
        "BOTH",
        240_000_000,
        Some(start_at),
        None,
    )
    .await
    .expect("create_program");
    assert!(program.active);

    db::rewards::distribute_all_programs(&pool, TEST_PRICE_MAX_STALE)
        .await
        .expect("distribute");

    let rewards_a = db::rewards::get_user_rewards(&pool, user_a).await.unwrap();
    let rewards_b = db::rewards::get_user_rewards(&pool, user_b).await.unwrap();
    assert_eq!(rewards_a.len(), 1);
    assert_eq!(rewards_b.len(), 1);
    assert!(
        rewards_a[0].received_total > rewards_b[0].received_total,
        "user A (3x weight) must receive more than B"
    );

    let a_ltc = common::wallet_balance(&pool, user_a_ltc).await;
    let b_ltc = common::wallet_balance(&pool, user_b_ltc).await;
    assert!(!a_ltc.is_zero());
    assert!(!b_ltc.is_zero());

    let a_before = rewards_a[0].received_total;
    db::rewards::distribute_all_programs(&pool, TEST_PRICE_MAX_STALE)
        .await
        .expect("second tick");
    let rewards_a_after = db::rewards::get_user_rewards(&pool, user_a).await.unwrap();
    assert!(
        rewards_a_after[0].received_total < a_before.saturating_mul(2),
        "immediate re-tick must not double-pay a full window"
    );

    db::rewards::set_program_active(&pool, program.id, false)
        .await
        .unwrap();
    let programs = db::rewards::list_programs(&pool).await.unwrap();
    let updated = programs.iter().find(|p| p.id == program.id).unwrap();
    assert!(!updated.active);
}

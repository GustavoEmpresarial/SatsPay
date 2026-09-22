//! Faucet cooldown clock and claim races, plus the merchant's accepted-coin
//! setting (empty = everything live, paused coins never offered).

mod common;

use bigdecimal::BigDecimal;
use db::faucet::{self, FaucetError};
use db::merchant_settings::{self as ms, MerchantSettingsError};
use shared::{coin_config, Coin};
use sqlx::PgPool;

#[sqlx::test(migrations = "../db/migrations")]
async fn cooldowns_track_each_coin_for_that_user_only(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    common::seed_house_liquidity(&pool, &[Coin::Pol], 10_000_000_000_000).await;
    let user = common::insert_user(&pool, "fc-cd").await;
    let other = common::insert_user(&pool, "fc-cd-other").await;
    common::insert_personal_wallet(&pool, user, Coin::Pol).await;

    let before = faucet::cooldowns(&pool, user, "203.0.113.1", 60).await.unwrap();
    assert_eq!(before.len(), shared::COINS.len());
    assert!(before.iter().all(|r| r.next_claim_at.is_none()), "nothing claimed yet");

    let claimed = faucet::claim(&pool, user, Coin::Pol, "203.0.113.1", 60).await.unwrap();
    let rows = faucet::cooldowns(&pool, user, "203.0.113.1", 60).await.unwrap();
    for r in &rows {
        if r.coin == "POL" {
            let next = r.next_claim_at.expect("POL is cooling down");
            assert!((next - claimed.next_claim_at).num_seconds().abs() <= 5, "{next} vs {}", claimed.next_claim_at);
        } else {
            assert!(r.next_claim_at.is_none(), "{} untouched", r.coin);
        }
    }
    // Another user on the same IP has their own clock.
    assert!(faucet::cooldowns(&pool, other, "203.0.113.1", 60).await.unwrap().iter().all(|r| r.next_claim_at.is_none()));

    // Once the window has passed the coin is ready again.
    sqlx::query("UPDATE faucet_claims SET created_at = now() - interval '2 hours'").execute(&pool).await.unwrap();
    assert!(faucet::cooldowns(&pool, user, "203.0.113.1", 60).await.unwrap().iter().all(|r| r.next_claim_at.is_none()));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn racing_claims_pay_once(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    common::seed_house_liquidity(&pool, &[Coin::Pol], 10_000_000_000_000).await;
    let user = common::insert_user(&pool, "fc-race").await;
    let wallet = common::insert_personal_wallet(&pool, user, Coin::Pol).await;

    let (a, b, c) = tokio::join!(
        faucet::claim(&pool, user, Coin::Pol, "203.0.113.2", 60),
        faucet::claim(&pool, user, Coin::Pol, "203.0.113.2", 60),
        faucet::claim(&pool, user, Coin::Pol, "203.0.113.2", 60),
    );
    let results = [a, b, c];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "exactly one claim wins");
    assert!(results.iter().filter_map(|r| r.as_ref().err()).all(|e| matches!(e, FaucetError::Cooldown { .. })));
    assert_eq!(common::wallet_balance(&pool, wallet).await, BigDecimal::from(coin_config(Coin::Pol).faucet_reward));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn claim_without_wallet_or_house_funds_pays_nothing(pool: PgPool) {
    let user = common::insert_user(&pool, "fc-none").await;
    let r = faucet::claim(&pool, user, Coin::Pol, "203.0.113.3", 60).await;
    assert!(matches!(r, Err(FaucetError::NotFound)), "{:?}", r.err());

    // Empty house: the claim rolls back, no claim row is left to start a cooldown.
    let wallet = common::insert_personal_wallet(&pool, user, Coin::Pol).await;
    let r = faucet::claim(&pool, user, Coin::Pol, "203.0.113.3", 60).await;
    assert!(matches!(r, Err(FaucetError::House(_))), "{:?}", r.err());
    assert_eq!(common::wallet_balance(&pool, wallet).await, BigDecimal::from(0));
    assert!(faucet::cooldowns(&pool, user, "203.0.113.3", 60).await.unwrap().iter().all(|c| c.next_claim_at.is_none()));
}

#[test]
fn ip_lock_key_is_stable_and_distinct() {
    assert_eq!(faucet::faucet_ip_lock_key("1.2.3.4", Coin::Btc), faucet::faucet_ip_lock_key("1.2.3.4", Coin::Btc));
    assert_ne!(faucet::faucet_ip_lock_key("1.2.3.4", Coin::Btc), faucet::faucet_ip_lock_key("1.2.3.4", Coin::Ltc));
    assert_ne!(faucet::faucet_ip_lock_key("1.2.3.4", Coin::Btc), faucet::faucet_ip_lock_key("1.2.3.5", Coin::Btc));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn accepted_coins_default_to_everything_live(pool: PgPool) {
    let merchant = common::insert_user(&pool, "ms-default").await;
    let live = ms::accepted_coins(&pool, merchant).await.unwrap();
    assert!(!live.is_empty());
    assert!(live.iter().all(|c| !shared::is_deposit_withdraw_paused(*c)), "paused coins never offered");
    assert_eq!(live, ms::live_coins(&[]));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_can_narrow_but_not_empty_the_checkout(pool: PgPool) {
    let merchant = common::insert_user(&pool, "ms-set").await;

    let saved = ms::set_accepted_coins(&pool, merchant, &[Coin::Usdt, Coin::Pol, Coin::Btc]).await.unwrap();
    assert!(!saved.contains(&Coin::Btc), "BTC is paused, filtered even though configured");
    assert!(saved.contains(&Coin::Usdt) && saved.contains(&Coin::Pol));
    assert_eq!(ms::accepted_coins(&pool, merchant).await.unwrap(), saved);

    // Only paused coins would leave nothing to pay with: rejected, previous setting kept.
    let r = ms::set_accepted_coins(&pool, merchant, &[Coin::Btc, Coin::Ltc]).await;
    assert!(matches!(r, Err(MerchantSettingsError::NoUsableCoin)), "{r:?}");
    assert_eq!(ms::accepted_coins(&pool, merchant).await.unwrap(), saved);

    // Clearing goes back to "everything live".
    let cleared = ms::set_accepted_coins(&pool, merchant, &[]).await.unwrap();
    assert_eq!(cleared, ms::live_coins(&[]));
}

mod common;

use bigdecimal::BigDecimal;
use shared::Coin;
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn list_and_transfer_between_kinds(pool: PgPool) {
    let user = common::insert_user(&pool, "walletsqlx").await;
    let personal = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    let _dev: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'DEVELOPER') RETURNING id",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    common::credit_wallet(&pool, personal, 5_000_000, "seed").await;

    let wallets = db::wallet::list_wallets(&pool, user, "PERSONAL").await.unwrap();
    assert!(wallets.iter().any(|w| w.coin == "BTC"));
    assert!(wallets.iter().any(|w| w.balance > BigDecimal::from(0u32)));

    db::wallet::transfer_between_kinds(&pool, user, "BTC", BigDecimal::from(1_000_000u64), true)
        .await
        .unwrap();
    let personal_bal = common::wallet_balance(&pool, personal).await;
    assert_eq!(personal_bal, BigDecimal::from(4_000_000u64));

    let entries = db::wallet::list_ledger_entries(&pool, user, "PERSONAL", Some("BTC"), 10)
        .await
        .unwrap();
    assert!(!entries.is_empty());
}

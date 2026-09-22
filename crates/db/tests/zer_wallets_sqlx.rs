//! Migration 0031/0032: ZER enum, HD seq, wallets via create_user_with_wallets.

mod common;

use domain::auth::AuthRepo;
use shared::COINS;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn zer_hd_seq_and_enum_accept_insert(pool: PgPool) {
    let seq: i64 = sqlx::query_scalar("SELECT nextval('zer_hd_index_seq')")
        .fetch_one(&pool)
        .await
        .expect("zer_hd_index_seq from 0032");
    assert!(seq >= 1);

    let user = common::insert_user(&pool, "zer-enum").await;
    let wallet_id: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'ZER', 'PERSONAL') RETURNING id",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .expect("coin enum includes ZER");
    assert!(!wallet_id.is_nil());
}

#[sqlx::test(migrations = "./migrations")]
async fn create_user_with_wallets_includes_zer(pool: PgPool) {
    let repo = db::auth::PgAuthRepo::new(pool.clone());
    let email = format!("zer-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("z{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let hash = crypto::hash_password("x").unwrap();

    let user = repo
        .create_user_with_wallets(&email, &username, &hash, &COINS)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM wallets WHERE user_id = $1 AND coin = 'ZER'",
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2, "PERSONAL + DEVELOPER");

    let coins: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT coin::text FROM wallets WHERE user_id = $1 ORDER BY 1",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(coins.iter().any(|c| c == "ZER"));
    assert_eq!(coins.len(), COINS.len());
}

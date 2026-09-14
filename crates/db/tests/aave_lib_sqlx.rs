//! Aave GraphQL sync via wiremock + db::connect smoke.

mod common;

use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[sqlx::test(migrations = "./migrations")]
async fn aave_sync_live_parses_markets(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "markets": [
                    {
                        "currency": { "symbol": "WBTC", "name": "Wrapped BTC", "decimals": 8 },
                        "supplyApy": 0.038,
                        "variableBorrowApy": 0.059,
                        "utilizationRate": 0.62,
                        "totalLiquidity": 1.2e8
                    },
                    {
                        "currency": { "symbol": "USDC", "name": "USD Coin", "decimals": 6 },
                        "supplyApy": 0.085,
                        "variableBorrowApy": 0.109,
                        "utilizationRate": 0.75,
                        "totalLiquidity": 5.2e7
                    },
                    {
                        "currency": { "symbol": "UNKNOWN", "name": "X", "decimals": 18 },
                        "supplyApy": 0.1,
                        "variableBorrowApy": 0.2,
                        "utilizationRate": 0.5,
                        "totalLiquidity": 1.0
                    },
                    {
                        "currency": { "symbol": "WMATIC", "name": "Wrapped MATIC", "decimals": 18 },
                        "supplyApy": 0.054,
                        "variableBorrowApy": 0.078,
                        "utilizationRate": 0.55,
                        "totalLiquidity": 1.8e7
                    },
                    {
                        "currency": { "symbol": "USDT", "name": "Tether", "decimals": 6 },
                        "supplyApy": 0.087,
                        "variableBorrowApy": 0.112,
                        "utilizationRate": 0.78,
                        "totalLiquidity": 4.5e7
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let url = format!("{}/graphql", server.uri());
    db::aave_sync::sync_live_aave_rates_at(&pool, &http, &url)
        .await
        .unwrap();

    let rates = db::aave_sync::get_aave_rates(&pool).await.unwrap();
    assert!(rates.contains_key(&shared::Coin::Btc));
    assert!(rates.contains_key(&shared::Coin::Usdc));

    db::aave_sync::sync_live_aave_rates_at(&pool, &http, "http://127.0.0.1:1/graphql")
        .await
        .unwrap();

    // Default URL wrapper (network may fail — still Ok)
    let _ = db::aave_sync::sync_live_aave_rates(&pool, &http).await;

    // Non-success HTTP still Ok
    Mock::given(method("POST"))
        .and(path("/fail"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    db::aave_sync::sync_live_aave_rates_at(&pool, &http, &format!("{}/fail", server.uri()))
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn db_connect_and_migrations_ok(pool: PgPool) {
    // pool already migrated by sqlx::test; also exercise connect helper against same URL
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let p2 = db::connect(&url).await.unwrap();
    db::run_migrations(&p2).await.unwrap();
    let _ = pool; // keep migrator pool alive
}

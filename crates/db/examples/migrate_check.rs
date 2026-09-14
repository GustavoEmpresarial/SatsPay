//! CI migration dry-run: connects to `DATABASE_URL` and runs every
//! migration end to end (`cargo run -p db --example migrate_check`). Used by
//! `.github/workflows/ci.yml` against an ephemeral Postgres service
//! container — a real migration run, not a syntax check.

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("connect");
    db::run_migrations(&pool).await.expect("run_migrations");
    println!("migrations applied cleanly");
}

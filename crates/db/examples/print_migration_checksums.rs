//! One-off helper: prints the checksum sqlx computes for each migration file,
//! so a manually-edited migration's `_sqlx_migrations.checksum` can be
//! updated in place instead of wiping the dev database.
fn main() {
    let migrator = sqlx::migrate!("./migrations");
    for m in migrator.iter() {
        println!("version={} checksum={}", m.version, hex::encode(&*m.checksum));
    }
}

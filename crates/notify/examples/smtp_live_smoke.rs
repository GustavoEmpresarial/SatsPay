//! Manual smoke test — sends a REAL email over SMTP via an Ethereal test
//! account (not run in CI). `cargo run -p notify --example smtp_live_smoke`
//! with `SMTP_HOST`/`SMTP_PORT`/`SMTP_USERNAME`/`SMTP_PASSWORD`/`SMTP_FROM_ADDRESS`
//! set to a real (test) mailbox's credentials.

use domain::auth::EmailSender;
use notify::{SmtpConfig, SmtpSender};

#[tokio::main]
async fn main() {
    let config = SmtpConfig {
        host: std::env::var("SMTP_HOST").expect("SMTP_HOST"),
        port: std::env::var("SMTP_PORT").expect("SMTP_PORT").parse().expect("SMTP_PORT must be numeric"),
        username: std::env::var("SMTP_USERNAME").expect("SMTP_USERNAME"),
        password: std::env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD"),
        from_address: std::env::var("SMTP_FROM_ADDRESS").expect("SMTP_FROM_ADDRESS"),
    };
    let to = std::env::var("SMTP_TEST_TO").expect("SMTP_TEST_TO");

    let sender = SmtpSender::new(config).expect("build SMTP transport");
    sender.send(&to, "BitcoSats SMTP live smoke test", "This is a real email sent over real SMTP during development — proves EmailSender::send actually delivers, not just compiles.").await.expect("send email");

    println!("EMAIL SENT SUCCESSFULLY (real SMTP delivery)");
}

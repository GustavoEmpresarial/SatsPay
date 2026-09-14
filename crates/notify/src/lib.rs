//! Real SMTP `EmailSender` implementation, using `lettre`.

use async_trait::async_trait;
use domain::auth::EmailSender;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Debug, thiserror::Error)]
pub enum SmtpError {
    #[error("invalid SMTP config: {0}")]
    Config(String),
    #[error("failed to send email: {0}")]
    Send(String),
}

pub struct SmtpSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from_address: String,
}

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
}

impl SmtpSender {
    pub fn new(config: SmtpConfig) -> Result<Self, SmtpError> {
        let creds = Credentials::new(config.username, config.password);
        // STARTTLS (port 587 by convention, but driven by whatever `config.port`
        // actually is): the connection starts in plaintext and upgrades to TLS
        // via the STARTTLS command — `relay()` instead assumes implicit TLS from
        // the first byte (port 465 style) and fails the handshake on 587.
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host).map_err(|e| SmtpError::Config(e.to_string()))?.port(config.port).credentials(creds).build();
        Ok(Self { transport, from_address: config.from_address })
    }
}

#[async_trait]
impl EmailSender for SmtpSender {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        let message = Message::builder()
            .from(self.from_address.parse().map_err(|e: lettre::address::AddressError| e.to_string())?)
            .to(to.parse().map_err(|e: lettre::address::AddressError| e.to_string())?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())
            .map_err(|e| e.to_string())?;

        self.transport.send(message).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smtp_new_builds_transport_without_connecting() {
        // Does not connect — only builds the STARTTLS transport client.
        let sender = SmtpSender::new(SmtpConfig {
            host: "smtp.example.test".into(),
            port: 587,
            username: "u".into(),
            password: "p".into(),
            from_address: "noreply@bitcosats.test".into(),
        });
        assert!(sender.is_ok());
    }
}

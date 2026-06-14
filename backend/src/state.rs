use anyhow::Result;
use sqlx::PgPool;

use crate::{config::Config, email::EmailSender, stripe::StripeClient};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub session_ttl_seconds: i64,
    pub password_reset_ttl_seconds: i64,
    pub mfa_challenge_ttl_seconds: i64,
    pub mfa_encryption_key: [u8; 32],
    pub cookie_secure: bool,
    pub email_sender: EmailSender,
    pub app_base_url: String,
    pub stripe: StripeClient,
}

impl AppState {
    pub fn new(pool: PgPool, config: &Config) -> Result<Self> {
        Ok(Self {
            pool,
            session_ttl_seconds: config.session_ttl_seconds,
            password_reset_ttl_seconds: config.password_reset_ttl_seconds,
            mfa_challenge_ttl_seconds: config.mfa_challenge_ttl_seconds,
            mfa_encryption_key: config.mfa_encryption_key,
            cookie_secure: config.cookie_secure,
            email_sender: EmailSender::new(config)?,
            app_base_url: config.app_base_url.trim_end_matches('/').to_owned(),
            stripe: StripeClient::new(config),
        })
    }
}

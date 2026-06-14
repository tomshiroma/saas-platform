use anyhow::Result;
use sqlx::PgPool;

use crate::{config::Config, email::EmailSender};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub session_ttl_seconds: i64,
    pub password_reset_ttl_seconds: i64,
    pub cookie_secure: bool,
    pub email_sender: EmailSender,
}

impl AppState {
    pub fn new(pool: PgPool, config: &Config) -> Result<Self> {
        Ok(Self {
            pool,
            session_ttl_seconds: config.session_ttl_seconds,
            password_reset_ttl_seconds: config.password_reset_ttl_seconds,
            cookie_secure: config.cookie_secure,
            email_sender: EmailSender::new(config)?,
        })
    }
}

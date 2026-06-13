use sqlx::PgPool;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub session_ttl_seconds: i64,
    pub cookie_secure: bool,
}

impl AppState {
    pub fn new(pool: PgPool, config: &Config) -> Self {
        Self {
            pool,
            session_ttl_seconds: config.session_ttl_seconds,
            cookie_secure: config.cookie_secure,
        }
    }
}

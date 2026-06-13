use anyhow::{Context, Result};

pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub session_ttl_seconds: i64,
    pub cookie_secure: bool,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL must be configured")?;
        let port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "3000".to_owned())
            .parse()
            .context("API_PORT must be a valid u16")?;
        let session_ttl_seconds = std::env::var("SESSION_TTL_SECONDS")
            .unwrap_or_else(|_| "28800".to_owned())
            .parse()
            .context("SESSION_TTL_SECONDS must be a valid i64")?;
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .unwrap_or_else(|_| "false".to_owned())
            .parse()
            .context("COOKIE_SECURE must be true or false")?;

        Ok(Self {
            database_url,
            port,
            session_ttl_seconds,
            cookie_secure,
        })
    }
}

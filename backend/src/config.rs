use anyhow::{Context, Result};

pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub session_ttl_seconds: i64,
    pub password_reset_ttl_seconds: i64,
    pub cookie_secure: bool,
    pub app_base_url: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_from: String,
    pub stripe_secret_key: String,
    pub stripe_webhook_secret: String,
    pub stripe_api_base_url: String,
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
        let password_reset_ttl_seconds = std::env::var("PASSWORD_RESET_TTL_SECONDS")
            .unwrap_or_else(|_| "1800".to_owned())
            .parse()
            .context("PASSWORD_RESET_TTL_SECONDS must be a valid i64")?;
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .unwrap_or_else(|_| "false".to_owned())
            .parse()
            .context("COOKIE_SECURE must be true or false")?;
        let app_base_url =
            std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:5173".to_owned());
        let smtp_host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "mailpit".to_owned());
        let smtp_port = std::env::var("SMTP_PORT")
            .unwrap_or_else(|_| "1025".to_owned())
            .parse()
            .context("SMTP_PORT must be a valid u16")?;
        let smtp_from = std::env::var("SMTP_FROM")
            .unwrap_or_else(|_| "SaaS Platform <no-reply@example.test>".to_owned());
        let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY").unwrap_or_default();
        let stripe_webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
        let stripe_api_base_url = std::env::var("STRIPE_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.stripe.com".to_owned());

        Ok(Self {
            database_url,
            port,
            session_ttl_seconds,
            password_reset_ttl_seconds,
            cookie_secure,
            app_base_url,
            smtp_host,
            smtp_port,
            smtp_from,
            stripe_secret_key,
            stripe_webhook_secret,
            stripe_api_base_url,
        })
    }
}

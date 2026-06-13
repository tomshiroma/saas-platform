use anyhow::{Context, Result};

pub struct Config {
    pub database_url: String,
    pub port: u16,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL must be configured")?;
        let port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "3000".to_owned())
            .parse()
            .context("API_PORT must be a valid u16")?;

        Ok(Self { database_url, port })
    }
}

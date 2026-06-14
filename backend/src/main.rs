mod app;
mod auth;
mod billing;
mod config;
mod email;
mod error;
mod mfa;
mod platform;
mod state;
mod stripe;
mod tenant;

use anyhow::{Context, Result};
use config::Config;
use sqlx::PgPool;
use state::AppState;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = Config::from_env()?;
    let pool = PgPool::connect(&config.database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    match std::env::args().nth(1).as_deref() {
        Some("migrate") => migrate(&pool).await,
        Some("seed") => seed(&pool).await,
        Some(command) => anyhow::bail!("unknown command: {command}"),
        None => serve(config, pool).await,
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).json().init();
}

async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::migrate!()
        .run(pool)
        .await
        .context("failed to run database migrations")?;
    info!("database migrations completed");
    Ok(())
}

async fn seed(pool: &PgPool) -> Result<()> {
    const TENANT_ID: Uuid = Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0001);
    const USER_ID: Uuid = Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0002);
    const PLATFORM_ADMIN_ID: Uuid = Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0010);

    let mut transaction = pool
        .begin()
        .await
        .context("failed to begin seed transaction")?;

    sqlx::query(
        "INSERT INTO tenants (id, slug, name) VALUES ($1, 'development', $2) ON CONFLICT (id) DO NOTHING",
    )
        .bind(TENANT_ID)
        .bind("開発テナント")
        .execute(&mut *transaction)
        .await
        .context("failed to seed tenant")?;

    sqlx::query("UPDATE tenants SET slug = 'development', name = '開発テナント' WHERE id = $1")
        .bind(TENANT_ID)
        .execute(&mut *transaction)
        .await
        .context("failed to update development tenant")?;

    sqlx::query("SELECT set_config('app.current_tenant_id', $1, true)")
        .bind(TENANT_ID.to_string())
        .execute(&mut *transaction)
        .await
        .context("failed to set tenant context")?;

    let password_hash = auth::hash_password("development-password".to_owned())
        .await
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;

    sqlx::query(
        r#"
        INSERT INTO users (id, tenant_id, email, display_name, role, password_hash)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (tenant_id, lower(email))
        DO UPDATE SET
            display_name = EXCLUDED.display_name,
            role = EXCLUDED.role,
            password_hash = EXCLUDED.password_hash,
            active = true,
            updated_at = now()
        "#,
    )
    .bind(USER_ID)
    .bind(TENANT_ID)
    .bind("admin@example.test")
    .bind("開発管理者")
    .bind("admin")
    .bind(password_hash)
    .execute(&mut *transaction)
    .await
    .context("failed to seed user")?;

    let platform_password_hash = auth::hash_password("platform-development-password".to_owned())
        .await
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    sqlx::query(
        r#"
        INSERT INTO platform_admins
            (id, email, display_name, password_hash, totp_secret)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (lower(email))
        DO UPDATE SET
            display_name = EXCLUDED.display_name,
            password_hash = EXCLUDED.password_hash,
            totp_secret = EXCLUDED.totp_secret,
            active = true,
            failed_login_count = 0,
            locked_until = NULL,
            updated_at = now()
        "#,
    )
    .bind(PLATFORM_ADMIN_ID)
    .bind("platform-admin@example.test")
    .bind("SaaS運営管理者")
    .bind(platform_password_hash)
    .bind(hex_to_bytes("48656c6c6f21deadbeef")?)
    .execute(&mut *transaction)
    .await
    .context("failed to seed platform administrator")?;

    transaction
        .commit()
        .await
        .context("failed to commit seed transaction")?;

    info!("development seed completed");
    Ok(())
}

fn hex_to_bytes(value: &str) -> Result<Vec<u8>> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(text, 16)?)
        })
        .collect()
}

async fn serve(config: Config, pool: PgPool) -> Result<()> {
    let address = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind to {address}"))?;
    let state = AppState::new(pool, &config)?;
    let router = app::router(state);

    info!(%address, "API server started");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("API server failed")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

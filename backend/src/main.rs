mod app;
mod config;

use anyhow::{Context, Result};
use config::Config;
use sqlx::PgPool;
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

    let mut transaction = pool
        .begin()
        .await
        .context("failed to begin seed transaction")?;

    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
        .bind(TENANT_ID)
        .bind("開発テナント")
        .execute(&mut *transaction)
        .await
        .context("failed to seed tenant")?;

    sqlx::query("SELECT set_config('app.current_tenant_id', $1, true)")
        .bind(TENANT_ID.to_string())
        .execute(&mut *transaction)
        .await
        .context("failed to set tenant context")?;

    sqlx::query(
        r#"
        INSERT INTO users (id, tenant_id, email, display_name, role)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (tenant_id, email) DO NOTHING
        "#,
    )
    .bind(USER_ID)
    .bind(TENANT_ID)
    .bind("admin@example.test")
    .bind("開発管理者")
    .bind("admin")
    .execute(&mut *transaction)
    .await
    .context("failed to seed user")?;

    transaction
        .commit()
        .await
        .context("failed to commit seed transaction")?;

    info!("development seed completed");
    Ok(())
}

async fn serve(config: Config, pool: PgPool) -> Result<()> {
    let address = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind to {address}"))?;
    let router = app::router(pool);

    info!(%address, "API server started");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("API server failed")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

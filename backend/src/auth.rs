use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

const SESSION_COOKIE: &str = "saas_session";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurrentUser {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub tenant_name: String,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub csrf_token: String,
}

impl CurrentUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

#[derive(Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub tenant_name: String,
    pub tenant_slug: String,
    pub display_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub tenant_slug: String,
    pub email: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    pub user: CurrentUser,
}

#[derive(sqlx::FromRow)]
struct LoginUser {
    id: Uuid,
    tenant_id: Uuid,
    tenant_slug: String,
    tenant_name: String,
    email: String,
    display_name: String,
    role: String,
    password_hash: String,
    failed_login_count: i32,
    locked: bool,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses((status = 201, description = "Tenant and administrator created", body = AuthResponse))
)]
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> ApiResult<Response> {
    let tenant_name = required_text(&request.tenant_name, "テナント名", 100)?;
    let tenant_slug = normalize_slug(&request.tenant_slug)?;
    let display_name = required_text(&request.display_name, "表示名", 100)?;
    let email = normalize_email(&request.email)?;
    validate_password(&request.password)?;
    let password_hash = hash_password(request.password).await?;

    let tenant_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;

    let tenant_insert = sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
        .bind(tenant_id)
        .bind(&tenant_slug)
        .bind(&tenant_name)
        .execute(&mut *transaction)
        .await;

    if let Err(error) = tenant_insert {
        if is_unique_violation(&error) {
            return Err(ApiError::conflict(
                "TENANT_SLUG_EXISTS",
                "このテナントIDは既に使用されています。",
            ));
        }
        return Err(ApiError::internal(error));
    }

    set_tenant_context(&mut transaction, tenant_id).await?;

    sqlx::query(
        r#"
        INSERT INTO users (id, tenant_id, email, display_name, role, password_hash)
        VALUES ($1, $2, $3, $4, 'admin', $5)
        "#,
    )
    .bind(user_id)
    .bind(tenant_id)
    .bind(&email)
    .bind(&display_name)
    .bind(password_hash)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    write_audit(
        &mut transaction,
        tenant_id,
        user_id,
        "tenant.register",
        "tenant",
        Some(tenant_id),
    )
    .await?;

    let (session_token, csrf_token) =
        create_session(&mut transaction, &state, tenant_id, user_id).await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let mut response = auth_response(
        &state,
        session_token,
        CurrentUser {
            id: user_id,
            tenant_id,
            tenant_slug,
            tenant_name,
            email,
            display_name,
            role: "admin".to_owned(),
            csrf_token,
        },
    );
    *response.status_mut() = axum::http::StatusCode::CREATED;
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses((status = 200, description = "Authenticated", body = AuthResponse))
)]
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Response> {
    let tenant_slug = normalize_slug(&request.tenant_slug)?;
    let email = normalize_email(&request.email)?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;

    let tenant =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM tenants WHERE slug = $1")
            .bind(&tenant_slug)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(invalid_credentials)?;

    set_tenant_context(&mut transaction, tenant.0).await?;
    let user = sqlx::query_as::<_, LoginUser>(
        r#"
        SELECT
            u.id,
            u.tenant_id,
            $2::text AS tenant_slug,
            $3::text AS tenant_name,
            u.email,
            u.display_name,
            u.role,
            u.password_hash,
            u.failed_login_count,
            coalesce(u.locked_until > now(), false) AS locked
        FROM users u
        WHERE u.tenant_id = $1 AND lower(u.email) = lower($4) AND u.active = true
        "#,
    )
    .bind(tenant.0)
    .bind(&tenant_slug)
    .bind(&tenant.1)
    .bind(&email)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(invalid_credentials)?;

    if user.locked {
        return Err(ApiError::too_many_requests(
            "ACCOUNT_LOCKED",
            "ログイン失敗回数が上限に達しました。15分後に再試行してください。",
        ));
    }

    if !verify_password(request.password, user.password_hash).await? {
        sqlx::query(
            r#"
            UPDATE users
            SET
                failed_login_count = failed_login_count + 1,
                locked_until = CASE
                    WHEN failed_login_count + 1 >= 5 THEN now() + interval '15 minutes'
                    ELSE locked_until
                END,
                updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
        transaction.commit().await.map_err(ApiError::internal)?;
        return Err(invalid_credentials());
    }

    if user.failed_login_count > 0 {
        sqlx::query(
            r#"
            UPDATE users
            SET failed_login_count = 0, locked_until = NULL, updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    }

    sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;

    let (session_token, csrf_token) =
        create_session(&mut transaction, &state, user.tenant_id, user.id).await?;
    write_audit(
        &mut transaction,
        user.tenant_id,
        user.id,
        "auth.login",
        "user",
        Some(user.id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    Ok(auth_response(
        &state,
        session_token,
        CurrentUser {
            id: user.id,
            tenant_id: user.tenant_id,
            tenant_slug: user.tenant_slug,
            tenant_name: user.tenant_name,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            csrf_token,
        },
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses((status = 200, description = "Current session", body = AuthResponse))
)]
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<AuthResponse>> {
    let user = authenticate(&state, &headers).await?;
    Ok(Json(AuthResponse { user }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses((status = 204, description = "Logged out"))
)]
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let user = authenticate(&state, &headers).await?;
    require_csrf(&headers, &user)?;
    let token = session_cookie(&headers).ok_or_else(ApiError::unauthorized)?;

    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash(&token))
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    let secure = if state.cookie_secure { "; Secure" } else { "" };
    let cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}");
    let mut response = axum::http::StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
    );
    Ok(response)
}

pub async fn authenticate(state: &AppState, headers: &HeaderMap) -> ApiResult<CurrentUser> {
    let token = session_cookie(headers).ok_or_else(ApiError::unauthorized)?;
    let session = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        r#"
        SELECT tenant_id, user_id, csrf_token
        FROM sessions
        WHERE token_hash = $1 AND expires_at > now()
        "#,
    )
    .bind(token_hash(&token))
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(ApiError::unauthorized)?;

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, session.0).await?;
    let user = sqlx::query_as::<_, (Uuid, Uuid, String, String, String, String, String)>(
        r#"
        SELECT u.id, u.tenant_id, t.slug, t.name, u.email, u.display_name, u.role
        FROM users u
        JOIN tenants t ON t.id = u.tenant_id
        WHERE u.id = $1 AND u.tenant_id = $2 AND u.active = true
        "#,
    )
    .bind(session.1)
    .bind(session.0)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(ApiError::unauthorized)?;
    sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE token_hash = $1")
        .bind(token_hash(&token))
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    transaction.commit().await.map_err(ApiError::internal)?;

    Ok(CurrentUser {
        id: user.0,
        tenant_id: user.1,
        tenant_slug: user.2,
        tenant_name: user.3,
        email: user.4,
        display_name: user.5,
        role: user.6,
        csrf_token: session.2,
    })
}

pub fn require_csrf(headers: &HeaderMap, user: &CurrentUser) -> ApiResult<()> {
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if supplied == Some(user.csrf_token.as_str()) {
        Ok(())
    } else {
        Err(ApiError::forbidden("CSRFトークンが不正です。"))
    }
}

pub async fn set_tenant_context(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> ApiResult<()> {
    sqlx::query("SELECT set_config('app.current_tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut **transaction)
        .await
        .map_err(ApiError::internal)?;
    Ok(())
}

pub async fn write_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_user_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
) -> ApiResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs
            (id, tenant_id, actor_user_id, action, target_type, target_id, result)
        VALUES ($1, $2, $3, $4, $5, $6, 'success')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

pub async fn hash_password(password: String) -> ApiResult<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(ApiError::internal)
    })
    .await
    .map_err(ApiError::internal)?
}

async fn verify_password(password: String, hash: String) -> ApiResult<bool> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash).map_err(ApiError::internal)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(ApiError::internal)?
}

async fn create_session(
    transaction: &mut Transaction<'_, Postgres>,
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<(String, String)> {
    let mut token_bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut token_bytes);
    let token = URL_SAFE_NO_PAD.encode(token_bytes);
    let csrf_token = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO sessions
            (id, tenant_id, user_id, token_hash, csrf_token, expires_at)
        VALUES ($1, $2, $3, $4, $5, now() + make_interval(secs => $6))
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(user_id)
    .bind(token_hash(&token))
    .bind(&csrf_token)
    .bind(state.session_ttl_seconds as f64)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;

    Ok((token, csrf_token))
}

fn auth_response(state: &AppState, session_token: String, user: CurrentUser) -> Response {
    let secure = if state.cookie_secure { "; Secure" } else { "" };
    let cookie = format!(
        "{SESSION_COOKIE}={session_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
        state.session_ttl_seconds
    );
    let mut response = Json(AuthResponse { user }).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("session cookie contains valid header characters"),
    );
    response
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.to_owned())
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn normalize_slug(value: &str) -> ApiResult<String> {
    let value = value.trim().to_ascii_lowercase();
    let valid = (4..=50).contains(&value.len())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && !value.starts_with('-')
        && !value.ends_with('-');
    if valid {
        Ok(value)
    } else {
        Err(ApiError::bad_request(
            "INVALID_TENANT_SLUG",
            "テナントIDは4～50文字の英小文字、数字、ハイフンで入力してください。",
        ))
    }
}

fn normalize_email(value: &str) -> ApiResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() <= 254 && value.contains('@') {
        Ok(value)
    } else {
        Err(ApiError::bad_request(
            "INVALID_EMAIL",
            "メールアドレスを正しく入力してください。",
        ))
    }
}

pub fn required_text(value: &str, label: &str, max_length: usize) -> ApiResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_length {
        Err(ApiError::bad_request(
            "INVALID_INPUT",
            format!("{label}は1～{max_length}文字で入力してください。"),
        ))
    } else {
        Ok(value.to_owned())
    }
}

pub fn validate_password(password: &str) -> ApiResult<()> {
    if password.len() < 12 || password.len() > 128 {
        Err(ApiError::bad_request(
            "INVALID_PASSWORD",
            "パスワードは12～128文字で入力してください。",
        ))
    } else {
        Ok(())
    }
}

fn invalid_credentials() -> ApiError {
    ApiError::unauthorized_with(
        "INVALID_CREDENTIALS",
        "テナントID、メールアドレス、またはパスワードが正しくありません。",
    )
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(database_error)
            if database_error.code().as_deref() == Some("23505")
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_email, normalize_slug, validate_password};

    #[test]
    fn tenant_slug_is_normalized() {
        assert_eq!(
            normalize_slug(" Example-Tenant ").unwrap(),
            "example-tenant"
        );
        assert!(normalize_slug("-invalid").is_err());
        assert!(normalize_slug("ab").is_err());
    }

    #[test]
    fn email_is_normalized() {
        assert_eq!(
            normalize_email(" Admin@Example.TEST ").unwrap(),
            "admin@example.test"
        );
        assert!(normalize_email("invalid").is_err());
    }

    #[test]
    fn password_requires_twelve_characters() {
        assert!(validate_password("123456789012").is_ok());
        assert!(validate_password("short").is_err());
    }
}

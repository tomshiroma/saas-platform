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
    mfa,
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
    pub mfa_enabled: bool,
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

#[derive(Deserialize, ToSchema)]
pub struct MfaCodeRequest {
    pub challenge_token: String,
    pub code: String,
}

#[derive(Deserialize, ToSchema)]
pub struct MfaResetRequest {
    pub password: String,
    pub code: String,
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordResetRequest {
    pub tenant_slug: String,
    pub email: String,
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordResetConfirmRequest {
    pub token: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct PasswordResetResponse {
    pub message: &'static str,
}

#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    pub user: CurrentUser,
}

#[derive(Serialize, ToSchema)]
pub struct AuthFlowResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<CurrentUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenge_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioning_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_codes: Option<Vec<String>>,
}

const PASSWORD_RESET_RESPONSE: &str =
    "入力された情報に一致するアカウントがある場合、再設定メールを送信しました。";

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
    mfa_secret_encrypted: Option<Vec<u8>>,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses((status = 201, description = "Tenant and administrator created; MFA setup required", body = AuthFlowResponse))
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

    let setup = create_mfa_setup_challenge(
        &mut transaction,
        &state,
        tenant_id,
        user_id,
        &email,
        &tenant_name,
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let mut response = Json(setup).into_response();
    *response.status_mut() = axum::http::StatusCode::CREATED;
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses((status = 200, description = "Authenticated or MFA challenge issued", body = AuthFlowResponse))
)]
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Response> {
    let tenant_slug = normalize_slug(&request.tenant_slug)?;
    let email = normalize_email(&request.email)?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;

    let tenant = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM tenants WHERE slug = $1 AND active = true",
    )
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
            coalesce(u.locked_until > now(), false) AS locked,
            u.mfa_secret_encrypted
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

    if !verify_password(request.password, user.password_hash.clone()).await? {
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

    if user.failed_login_count > 0 && user.role != "admin" {
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

    if user.role == "admin" {
        let response = if user.mfa_secret_encrypted.is_some() {
            create_mfa_verify_challenge(&mut transaction, &state, user.tenant_id, user.id).await?
        } else {
            create_mfa_setup_challenge(
                &mut transaction,
                &state,
                user.tenant_id,
                user.id,
                &user.email,
                &user.tenant_name,
            )
            .await?
        };
        transaction.commit().await.map_err(ApiError::internal)?;
        return Ok(Json(response).into_response());
    }

    let (session_token, csrf_token) =
        create_session(&mut transaction, &state, user.tenant_id, user.id, false).await?;
    let current_user = current_user_from_login(user, csrf_token, false);
    write_audit(
        &mut transaction,
        current_user.tenant_id,
        current_user.id,
        "auth.login",
        "user",
        Some(current_user.id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    Ok(auth_flow_response(&state, session_token, current_user))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/mfa/setup/confirm",
    request_body = MfaCodeRequest,
    responses((status = 200, description = "MFA setup completed", body = AuthFlowResponse))
)]
pub async fn confirm_mfa_setup(
    State(state): State<AppState>,
    Json(request): Json<MfaCodeRequest>,
) -> ApiResult<Response> {
    complete_mfa_challenge(&state, request, "setup").await
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/mfa/verify",
    request_body = MfaCodeRequest,
    responses((status = 200, description = "MFA verification completed", body = AuthFlowResponse))
)]
pub async fn verify_mfa(
    State(state): State<AppState>,
    Json(request): Json<MfaCodeRequest>,
) -> ApiResult<Response> {
    complete_mfa_challenge(&state, request, "verify").await
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/mfa/reset",
    request_body = MfaResetRequest,
    responses((status = 200, description = "Existing MFA removed and new setup challenge issued", body = AuthFlowResponse))
)]
pub async fn reset_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<MfaResetRequest>,
) -> ApiResult<Json<AuthFlowResponse>> {
    let user = authenticate(&state, &headers).await?;
    if !user.is_admin() {
        return Err(ApiError::forbidden("管理者権限が必要です。"));
    }
    require_csrf(&headers, &user)?;

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, user.tenant_id).await?;
    let credentials = sqlx::query_as::<_, (String, Option<Vec<u8>>)>(
        "SELECT password_hash, mfa_secret_encrypted FROM users WHERE id = $1 AND tenant_id = $2",
    )
    .bind(user.id)
    .bind(user.tenant_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    if !verify_password(request.password, credentials.0).await?
        || !verify_mfa_code(
            &mut transaction,
            &state,
            user.tenant_id,
            user.id,
            credentials.1.as_deref(),
            &request.code,
        )
        .await?
    {
        return Err(ApiError::unauthorized_with(
            "INVALID_MFA_CREDENTIALS",
            "パスワードまたは認証コードが正しくありません。",
        ));
    }

    sqlx::query(
        "UPDATE users SET mfa_secret_encrypted = NULL, mfa_enabled_at = NULL, updated_at = now() WHERE id = $1 AND tenant_id = $2",
    )
    .bind(user.id)
    .bind(user.tenant_id)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM user_mfa_recovery_codes WHERE user_id = $1 AND tenant_id = $2")
        .bind(user.id)
        .bind(user.tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND tenant_id = $2")
        .bind(user.id)
        .bind(user.tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    write_audit(
        &mut transaction,
        user.tenant_id,
        user.id,
        "auth.mfa_reset",
        "user",
        Some(user.id),
    )
    .await?;
    let response = create_mfa_setup_challenge(
        &mut transaction,
        &state,
        user.tenant_id,
        user.id,
        &user.email,
        &user.tenant_name,
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password-reset/request",
    request_body = PasswordResetRequest,
    responses((status = 202, description = "Password reset request accepted", body = PasswordResetResponse))
)]
pub async fn request_password_reset(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<PasswordResetResponse>)> {
    let tenant_slug = normalize_slug(&request.tenant_slug)?;
    let email = normalize_email(&request.email)?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;

    let tenant = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM tenants WHERE slug = $1 AND active = true",
    )
    .bind(&tenant_slug)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    let Some((tenant_id, tenant_name)) = tenant else {
        return Ok(password_reset_accepted());
    };

    set_tenant_context(&mut transaction, tenant_id).await?;
    let user_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM users
        WHERE tenant_id = $1 AND lower(email) = lower($2) AND active = true
        "#,
    )
    .bind(tenant_id)
    .bind(&email)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    let Some(user_id) = user_id else {
        return Ok(password_reset_accepted());
    };

    let recently_requested = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM password_reset_tokens
            WHERE user_id = $1
              AND used_at IS NULL
              AND created_at > now() - interval '1 minute'
        )
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    if recently_requested {
        return Ok(password_reset_accepted());
    }

    sqlx::query(
        "UPDATE password_reset_tokens SET used_at = now() WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    let token = password_reset_token(tenant_id);
    sqlx::query(
        r#"
        INSERT INTO password_reset_tokens
            (id, tenant_id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4, now() + make_interval(secs => $5))
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(user_id)
    .bind(token_hash(&token))
    .bind(state.password_reset_ttl_seconds as f64)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    write_audit(
        &mut transaction,
        tenant_id,
        user_id,
        "auth.password_reset_requested",
        "user",
        Some(user_id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    if let Err(error) = state
        .email_sender
        .send_password_reset(&email, &tenant_name, &token)
        .await
    {
        tracing::error!(%error, %user_id, "failed to send password reset email");
    }

    Ok(password_reset_accepted())
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password-reset/confirm",
    request_body = PasswordResetConfirmRequest,
    responses(
        (status = 204, description = "Password reset completed"),
        (status = 400, description = "Token is invalid or expired")
    )
)]
pub async fn confirm_password_reset(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetConfirmRequest>,
) -> ApiResult<axum::http::StatusCode> {
    let token = request.token.trim();
    if !(32..=128).contains(&token.len()) {
        return Err(invalid_reset_token());
    }
    validate_password(&request.password)?;
    let password_hash = hash_password(request.password).await?;

    let tenant_id = token
        .split_once('.')
        .and_then(|(value, _)| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_reset_token)?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, tenant_id).await?;

    let reset = sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"
        SELECT id, user_id
        FROM password_reset_tokens
        WHERE tenant_id = $1
          AND token_hash = $2
          AND used_at IS NULL
          AND expires_at > now()
        "#,
    )
    .bind(tenant_id)
    .bind(token_hash(token))
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(invalid_reset_token)?;

    let claimed = sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET used_at = now()
        WHERE id = $1 AND used_at IS NULL AND expires_at > now()
        "#,
    )
    .bind(reset.0)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    if claimed.rows_affected() != 1 {
        return Err(invalid_reset_token());
    }

    let updated = sqlx::query(
        r#"
        UPDATE users
        SET
            password_hash = $1,
            failed_login_count = 0,
            locked_until = NULL,
            updated_at = now()
        WHERE id = $2 AND tenant_id = $3 AND active = true
        "#,
    )
    .bind(password_hash)
    .bind(reset.1)
    .bind(tenant_id)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    if updated.rows_affected() != 1 {
        return Err(invalid_reset_token());
    }

    sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND tenant_id = $2")
        .bind(reset.1)
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET used_at = coalesce(used_at, now())
        WHERE user_id = $1 AND id <> $2
        "#,
    )
    .bind(reset.1)
    .bind(reset.0)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    write_audit(
        &mut transaction,
        tenant_id,
        reset.1,
        "auth.password_reset",
        "user",
        Some(reset.1),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
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

async fn complete_mfa_challenge(
    state: &AppState,
    request: MfaCodeRequest,
    expected_purpose: &str,
) -> ApiResult<Response> {
    let tenant_id = challenge_tenant_id(&request.challenge_token)?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, tenant_id).await?;
    let challenge = sqlx::query_as::<_, (Uuid, Uuid, String, Option<Vec<u8>>)>(
        r#"
        SELECT c.id, c.user_id, c.purpose, c.secret_encrypted
        FROM user_mfa_challenges c
        JOIN users u ON u.id = c.user_id AND u.tenant_id = c.tenant_id
        WHERE c.tenant_id = $1
          AND c.token_hash = $2
          AND c.expires_at > now()
          AND coalesce(u.locked_until <= now(), true)
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(token_hash(&request.challenge_token))
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(invalid_mfa_challenge)?;
    if challenge.2 != expected_purpose {
        return Err(invalid_mfa_challenge());
    }

    let user = load_current_user(&mut transaction, tenant_id, challenge.1, String::new()).await?;
    if !user.is_admin() {
        return Err(ApiError::forbidden("管理者権限が必要です。"));
    }

    let recovery_codes = if expected_purpose == "setup" {
        let encrypted = challenge.3.as_deref().ok_or_else(invalid_mfa_challenge)?;
        let secret = mfa::decrypt_secret(&state.mfa_encryption_key, encrypted)?;
        if !mfa::verify_totp(&secret, request.code.trim()) {
            record_mfa_failure(&mut transaction, tenant_id, user.id).await?;
            transaction.commit().await.map_err(ApiError::internal)?;
            return Err(invalid_mfa_code());
        }
        sqlx::query(
            "UPDATE users SET mfa_secret_encrypted = $1, mfa_enabled_at = now(), updated_at = now() WHERE id = $2 AND tenant_id = $3",
        )
        .bind(encrypted)
        .bind(user.id)
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
        sqlx::query("DELETE FROM user_mfa_recovery_codes WHERE user_id = $1 AND tenant_id = $2")
            .bind(user.id)
            .bind(tenant_id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
        let codes = mfa::generate_recovery_codes();
        for code in &codes {
            sqlx::query(
                "INSERT INTO user_mfa_recovery_codes (id, tenant_id, user_id, code_hash) VALUES ($1, $2, $3, $4)",
            )
            .bind(Uuid::now_v7())
            .bind(tenant_id)
            .bind(user.id)
            .bind(token_hash(&mfa::normalize_recovery_code(code)))
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
        }
        Some(codes)
    } else {
        let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT mfa_secret_encrypted FROM users WHERE id = $1 AND tenant_id = $2",
        )
        .bind(user.id)
        .bind(tenant_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(invalid_mfa_challenge)?;
        if !verify_mfa_code(
            &mut transaction,
            state,
            tenant_id,
            user.id,
            Some(&encrypted),
            &request.code,
        )
        .await?
        {
            record_mfa_failure(&mut transaction, tenant_id, user.id).await?;
            transaction.commit().await.map_err(ApiError::internal)?;
            return Err(invalid_mfa_code());
        }
        None
    };

    sqlx::query("DELETE FROM user_mfa_challenges WHERE id = $1")
        .bind(challenge.0)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query(
        "UPDATE users SET failed_login_count = 0, locked_until = NULL, updated_at = now() WHERE id = $1 AND tenant_id = $2",
    )
    .bind(user.id)
    .bind(tenant_id)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    let (session_token, csrf_token) =
        create_session(&mut transaction, state, tenant_id, user.id, true).await?;
    write_audit(
        &mut transaction,
        tenant_id,
        user.id,
        if expected_purpose == "setup" {
            "auth.mfa_setup"
        } else {
            "auth.login"
        },
        "user",
        Some(user.id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let mut authenticated_user = user;
    authenticated_user.csrf_token = csrf_token;
    authenticated_user.mfa_enabled = true;
    Ok(auth_flow_response_with_recovery_codes(
        state,
        session_token,
        authenticated_user,
        recovery_codes,
    ))
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
    let session = sqlx::query_as::<_, (Uuid, Uuid, String, bool)>(
        r#"
        SELECT tenant_id, user_id, csrf_token, mfa_verified
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
    let user = load_current_user(&mut transaction, session.0, session.1, session.2.clone()).await?;
    if user.is_admin() && !session.3 {
        return Err(ApiError::unauthorized());
    }
    sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE token_hash = $1")
        .bind(token_hash(&token))
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    transaction.commit().await.map_err(ApiError::internal)?;

    Ok(user)
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

pub async fn verify_password(password: String, hash: String) -> ApiResult<bool> {
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
    mfa_verified: bool,
) -> ApiResult<(String, String)> {
    let token = random_token();
    let csrf_token = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO sessions
            (id, tenant_id, user_id, token_hash, csrf_token, expires_at, mfa_verified)
        VALUES ($1, $2, $3, $4, $5, now() + make_interval(secs => $6), $7)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(user_id)
    .bind(token_hash(&token))
    .bind(&csrf_token)
    .bind(state.session_ttl_seconds as f64)
    .bind(mfa_verified)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;

    Ok((token, csrf_token))
}

fn auth_flow_response(state: &AppState, session_token: String, user: CurrentUser) -> Response {
    auth_flow_response_with_recovery_codes(state, session_token, user, None)
}

fn auth_flow_response_with_recovery_codes(
    state: &AppState,
    session_token: String,
    user: CurrentUser,
    recovery_codes: Option<Vec<String>>,
) -> Response {
    let secure = if state.cookie_secure { "; Secure" } else { "" };
    let cookie = format!(
        "{SESSION_COOKIE}={session_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
        state.session_ttl_seconds
    );
    let mut response = Json(AuthFlowResponse {
        status: "authenticated".to_owned(),
        user: Some(user),
        challenge_token: None,
        secret: None,
        provisioning_uri: None,
        recovery_codes,
    })
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("session cookie contains valid header characters"),
    );
    response
}

async fn create_mfa_setup_challenge(
    transaction: &mut Transaction<'_, Postgres>,
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
    email: &str,
    tenant_name: &str,
) -> ApiResult<AuthFlowResponse> {
    let secret = mfa::generate_secret();
    let encrypted = mfa::encrypt_secret(&state.mfa_encryption_key, &secret)?;
    let token = create_mfa_challenge(
        transaction,
        state,
        tenant_id,
        user_id,
        "setup",
        Some(encrypted),
    )
    .await?;
    Ok(AuthFlowResponse {
        status: "mfa_setup_required".to_owned(),
        user: None,
        challenge_token: Some(token),
        secret: Some(mfa::encode_secret(&secret)),
        provisioning_uri: Some(mfa::provisioning_uri(&secret, email, tenant_name)),
        recovery_codes: None,
    })
}

async fn create_mfa_verify_challenge(
    transaction: &mut Transaction<'_, Postgres>,
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<AuthFlowResponse> {
    let token =
        create_mfa_challenge(transaction, state, tenant_id, user_id, "verify", None).await?;
    Ok(AuthFlowResponse {
        status: "mfa_required".to_owned(),
        user: None,
        challenge_token: Some(token),
        secret: None,
        provisioning_uri: None,
        recovery_codes: None,
    })
}

async fn create_mfa_challenge(
    transaction: &mut Transaction<'_, Postgres>,
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
    purpose: &str,
    secret_encrypted: Option<Vec<u8>>,
) -> ApiResult<String> {
    sqlx::query("DELETE FROM user_mfa_challenges WHERE user_id = $1 OR expires_at <= now()")
        .bind(user_id)
        .execute(&mut **transaction)
        .await
        .map_err(ApiError::internal)?;
    let token = mfa::challenge_token(tenant_id);
    sqlx::query(
        r#"
        INSERT INTO user_mfa_challenges
            (id, tenant_id, user_id, token_hash, purpose, secret_encrypted, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, now() + make_interval(secs => $7))
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(user_id)
    .bind(token_hash(&token))
    .bind(purpose)
    .bind(secret_encrypted)
    .bind(state.mfa_challenge_ttl_seconds as f64)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(token)
}

async fn verify_mfa_code(
    transaction: &mut Transaction<'_, Postgres>,
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
    encrypted_secret: Option<&[u8]>,
    code: &str,
) -> ApiResult<bool> {
    let code = code.trim();
    if let Some(encrypted) = encrypted_secret {
        let secret = mfa::decrypt_secret(&state.mfa_encryption_key, encrypted)?;
        if mfa::verify_totp(&secret, code) {
            return Ok(true);
        }
    }

    let normalized = mfa::normalize_recovery_code(code);
    if normalized.len() < 8 {
        return Ok(false);
    }
    let used = sqlx::query(
        r#"
        UPDATE user_mfa_recovery_codes
        SET used_at = now()
        WHERE tenant_id = $1 AND user_id = $2 AND code_hash = $3 AND used_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(token_hash(&normalized))
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(used.rows_affected() == 1)
}

async fn record_mfa_failure(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<()> {
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
    .bind(user_id)
    .bind(tenant_id)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

async fn load_current_user(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    csrf_token: String,
) -> ApiResult<CurrentUser> {
    let user = sqlx::query_as::<_, (Uuid, Uuid, String, String, String, String, String, bool)>(
        r#"
        SELECT
            u.id,
            u.tenant_id,
            t.slug,
            t.name,
            u.email,
            u.display_name,
            u.role,
            u.mfa_enabled_at IS NOT NULL
        FROM users u
        JOIN tenants t ON t.id = u.tenant_id
        WHERE u.id = $1
          AND u.tenant_id = $2
          AND u.active = true
          AND t.active = true
        "#,
    )
    .bind(user_id)
    .bind(tenant_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(ApiError::unauthorized)?;
    Ok(CurrentUser {
        id: user.0,
        tenant_id: user.1,
        tenant_slug: user.2,
        tenant_name: user.3,
        email: user.4,
        display_name: user.5,
        role: user.6,
        mfa_enabled: user.7,
        csrf_token,
    })
}

fn current_user_from_login(user: LoginUser, csrf_token: String, mfa_enabled: bool) -> CurrentUser {
    CurrentUser {
        id: user.id,
        tenant_id: user.tenant_id,
        tenant_slug: user.tenant_slug,
        tenant_name: user.tenant_name,
        email: user.email,
        display_name: user.display_name,
        role: user.role,
        mfa_enabled,
        csrf_token,
    }
}

fn challenge_tenant_id(token: &str) -> ApiResult<Uuid> {
    token
        .split_once('.')
        .and_then(|(value, _)| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_mfa_challenge)
}

fn invalid_mfa_challenge() -> ApiError {
    ApiError::unauthorized_with(
        "INVALID_MFA_CHALLENGE",
        "MFA認証の有効期限が切れています。もう一度ログインしてください。",
    )
}

fn invalid_mfa_code() -> ApiError {
    ApiError::unauthorized_with(
        "INVALID_MFA_CODE",
        "認証コードまたはリカバリーコードが正しくありません。",
    )
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

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn random_token() -> String {
    let mut token_bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut token_bytes);
    URL_SAFE_NO_PAD.encode(token_bytes)
}

fn password_reset_token(tenant_id: Uuid) -> String {
    format!("{tenant_id}.{}", random_token())
}

fn password_reset_accepted() -> (axum::http::StatusCode, Json<PasswordResetResponse>) {
    (
        axum::http::StatusCode::ACCEPTED,
        Json(PasswordResetResponse {
            message: PASSWORD_RESET_RESPONSE,
        }),
    )
}

fn invalid_reset_token() -> ApiError {
    ApiError::bad_request(
        "INVALID_RESET_TOKEN",
        "再設定リンクが無効か、有効期限が切れています。もう一度申請してください。",
    )
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

use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha1::Sha1;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    auth::{random_token, set_tenant_context, token_hash, verify_password},
    error::{ApiError, ApiResult},
    state::AppState,
};

const PLATFORM_SESSION_COOKIE: &str = "saas_platform_session";
const TOTP_STEP_SECONDS: u64 = 30;
const TOTP_DIGITS: u32 = 1_000_000;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurrentPlatformAdmin {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub csrf_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct PlatformLoginRequest {
    pub email: String,
    pub password: String,
    pub totp_code: String,
}

#[derive(Serialize, ToSchema)]
pub struct PlatformAuthResponse {
    pub admin: CurrentPlatformAdmin,
}

#[derive(Serialize, ToSchema)]
pub struct PlatformSummaryResponse {
    pub tenant_count: usize,
    pub active_tenant_count: usize,
    pub suspended_tenant_count: usize,
    pub user_count: i64,
    pub active_user_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PlatformTenantResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub active: bool,
    pub suspension_reason: Option<String>,
    pub created_at: String,
    pub user_count: i64,
    pub active_user_count: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdatePlatformTenantRequest {
    pub active: bool,
    pub suspension_reason: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PlatformAuditLogResponse {
    pub id: Uuid,
    pub actor_display_name: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
struct PlatformLoginAdmin {
    id: Uuid,
    email: String,
    display_name: String,
    password_hash: String,
    totp_secret: Vec<u8>,
    locked: bool,
}

#[derive(sqlx::FromRow)]
struct PlatformTenantRow {
    id: Uuid,
    slug: String,
    name: String,
    active: bool,
    suspension_reason: Option<String>,
    created_at: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/platform/auth/login",
    request_body = PlatformLoginRequest,
    responses((status = 200, description = "Platform administrator authenticated", body = PlatformAuthResponse))
)]
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<PlatformLoginRequest>,
) -> ApiResult<Response> {
    let email = normalize_email(&request.email)?;
    let admin = sqlx::query_as::<_, PlatformLoginAdmin>(
        r#"
        SELECT
            id,
            email,
            display_name,
            password_hash,
            totp_secret,
            coalesce(locked_until > now(), false) AS locked
        FROM platform_admins
        WHERE lower(email) = lower($1) AND active = true
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(invalid_credentials)?;

    if admin.locked {
        return Err(ApiError::too_many_requests(
            "PLATFORM_ACCOUNT_LOCKED",
            "ログイン失敗回数が上限に達しました。15分後に再試行してください。",
        ));
    }

    let password_valid = verify_password(request.password, admin.password_hash).await?;
    let totp_valid = verify_totp(&admin.totp_secret, &request.totp_code);
    if !password_valid || !totp_valid {
        sqlx::query(
            r#"
            UPDATE platform_admins
            SET
                failed_login_count = failed_login_count + 1,
                locked_until = CASE
                    WHEN failed_login_count + 1 >= 5 THEN now() + interval '15 minutes'
                    ELSE locked_until
                END,
                updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(admin.id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
        return Err(invalid_credentials());
    }

    let token = random_token();
    let csrf_token = Uuid::new_v4().to_string();
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query(
        r#"
        UPDATE platform_admins
        SET failed_login_count = 0, locked_until = NULL, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(admin.id)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM platform_sessions WHERE expires_at <= now()")
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query(
        r#"
        INSERT INTO platform_sessions
            (id, admin_id, token_hash, csrf_token, expires_at)
        VALUES ($1, $2, $3, $4, now() + make_interval(secs => $5))
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(admin.id)
    .bind(token_hash(&token))
    .bind(&csrf_token)
    .bind(state.session_ttl_seconds as f64)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    write_platform_audit(
        &mut transaction,
        admin.id,
        "platform.login",
        "platform_admin",
        Some(admin.id),
        json!({}),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let secure = if state.cookie_secure { "; Secure" } else { "" };
    let cookie = format!(
        "{PLATFORM_SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{secure}",
        state.session_ttl_seconds
    );
    let mut response = Json(PlatformAuthResponse {
        admin: CurrentPlatformAdmin {
            id: admin.id,
            email: admin.email,
            display_name: admin.display_name,
            csrf_token,
        },
    })
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
    );
    Ok(response)
}

#[utoipa::path(
    get,
    path = "/api/v1/platform/auth/me",
    responses((status = 200, description = "Current platform administrator", body = PlatformAuthResponse))
)]
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<PlatformAuthResponse>> {
    let admin = authenticate(&state, &headers).await?;
    Ok(Json(PlatformAuthResponse { admin }))
}

#[utoipa::path(
    post,
    path = "/api/v1/platform/auth/logout",
    responses((status = 204, description = "Platform administrator logged out"))
)]
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let admin = authenticate(&state, &headers).await?;
    require_csrf(&headers, &admin)?;
    let token = session_cookie(&headers).ok_or_else(ApiError::unauthorized)?;
    sqlx::query("DELETE FROM platform_sessions WHERE token_hash = $1")
        .bind(token_hash(&token))
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    let secure = if state.cookie_secure { "; Secure" } else { "" };
    let cookie =
        format!("{PLATFORM_SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}");
    let mut response = axum::http::StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
    );
    Ok(response)
}

#[utoipa::path(
    get,
    path = "/api/v1/platform/summary",
    responses((status = 200, description = "Platform usage summary", body = PlatformSummaryResponse))
)]
pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<PlatformSummaryResponse>> {
    authenticate(&state, &headers).await?;
    let tenants = load_tenants(&state).await?;
    Ok(Json(PlatformSummaryResponse {
        tenant_count: tenants.len(),
        active_tenant_count: tenants.iter().filter(|tenant| tenant.active).count(),
        suspended_tenant_count: tenants.iter().filter(|tenant| !tenant.active).count(),
        user_count: tenants.iter().map(|tenant| tenant.user_count).sum(),
        active_user_count: tenants.iter().map(|tenant| tenant.active_user_count).sum(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/platform/tenants",
    responses((status = 200, description = "All customer tenants", body = [PlatformTenantResponse]))
)]
pub async fn list_tenants(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<PlatformTenantResponse>>> {
    authenticate(&state, &headers).await?;
    Ok(Json(load_tenants(&state).await?))
}

#[utoipa::path(
    patch,
    path = "/api/v1/platform/tenants/{tenant_id}",
    params(("tenant_id" = Uuid, Path, description = "Tenant ID")),
    request_body = UpdatePlatformTenantRequest,
    responses((status = 200, description = "Tenant status updated", body = PlatformTenantResponse))
)]
pub async fn update_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<Uuid>,
    Json(request): Json<UpdatePlatformTenantRequest>,
) -> ApiResult<Json<PlatformTenantResponse>> {
    let admin = authenticate(&state, &headers).await?;
    require_csrf(&headers, &admin)?;
    let reason = validate_suspension_reason(request.active, request.suspension_reason)?;

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    let updated = sqlx::query_as::<_, (String, String)>(
        r#"
        UPDATE tenants
        SET
            active = $1,
            suspended_at = CASE WHEN $1 THEN NULL ELSE now() END,
            suspension_reason = CASE WHEN $1 THEN NULL ELSE $2 END,
            updated_at = now()
        WHERE id = $3
        RETURNING slug, name
        "#,
    )
    .bind(request.active)
    .bind(&reason)
    .bind(tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("テナントが見つかりません。"))?;

    if !request.active {
        sqlx::query("DELETE FROM sessions WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        sqlx::query(
            "UPDATE password_reset_tokens SET used_at = coalesce(used_at, now()) WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    }

    write_platform_audit(
        &mut transaction,
        admin.id,
        if request.active {
            "tenant.reactivate"
        } else {
            "tenant.suspend"
        },
        "tenant",
        Some(tenant_id),
        json!({
            "tenant_slug": updated.0,
            "tenant_name": updated.1,
            "suspension_reason": reason,
        }),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let tenants = load_tenants(&state).await?;
    let tenant = tenants
        .into_iter()
        .find(|tenant| tenant.id == tenant_id)
        .ok_or_else(|| ApiError::not_found("テナントが見つかりません。"))?;
    Ok(Json(tenant))
}

#[utoipa::path(
    get,
    path = "/api/v1/platform/audit-logs",
    responses((status = 200, description = "Recent platform audit logs", body = [PlatformAuditLogResponse]))
)]
pub async fn audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<PlatformAuditLogResponse>>> {
    authenticate(&state, &headers).await?;
    let logs = sqlx::query_as::<
        _,
        (
            Uuid,
            Option<String>,
            String,
            String,
            Option<Uuid>,
            serde_json::Value,
            String,
        ),
    >(
        r#"
        SELECT
            l.id,
            a.display_name,
            l.action,
            l.target_type,
            l.target_id,
            l.details,
            to_char(l.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
        FROM platform_audit_logs l
        LEFT JOIN platform_admins a ON a.id = l.actor_admin_id
        ORDER BY l.created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .into_iter()
    .map(|row| PlatformAuditLogResponse {
        id: row.0,
        actor_display_name: row.1,
        action: row.2,
        target_type: row.3,
        target_id: row.4,
        details: row.5,
        created_at: row.6,
    })
    .collect();
    Ok(Json(logs))
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> ApiResult<CurrentPlatformAdmin> {
    let token = session_cookie(headers).ok_or_else(ApiError::unauthorized)?;
    let admin = sqlx::query_as::<_, (Uuid, String, String, String)>(
        r#"
        SELECT a.id, a.email, a.display_name, s.csrf_token
        FROM platform_sessions s
        JOIN platform_admins a ON a.id = s.admin_id
        WHERE s.token_hash = $1
          AND s.expires_at > now()
          AND a.active = true
        "#,
    )
    .bind(token_hash(&token))
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(ApiError::unauthorized)?;
    sqlx::query("UPDATE platform_sessions SET last_seen_at = now() WHERE token_hash = $1")
        .bind(token_hash(&token))
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    Ok(CurrentPlatformAdmin {
        id: admin.0,
        email: admin.1,
        display_name: admin.2,
        csrf_token: admin.3,
    })
}

async fn load_tenants(state: &AppState) -> ApiResult<Vec<PlatformTenantResponse>> {
    let rows = sqlx::query_as::<_, PlatformTenantRow>(
        r#"
        SELECT
            id,
            slug,
            name,
            active,
            suspension_reason,
            to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at
        FROM tenants
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let mut tenants = Vec::with_capacity(rows.len());
    for row in rows {
        let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
        set_tenant_context(&mut transaction, row.id).await?;
        let counts = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT count(*), count(*) FILTER (WHERE active = true)
            FROM users
            WHERE tenant_id = $1
            "#,
        )
        .bind(row.id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
        transaction.commit().await.map_err(ApiError::internal)?;

        tenants.push(PlatformTenantResponse {
            id: row.id,
            slug: row.slug,
            name: row.name,
            active: row.active,
            suspension_reason: row.suspension_reason,
            created_at: row.created_at,
            user_count: counts.0,
            active_user_count: counts.1,
        });
    }
    Ok(tenants)
}

async fn write_platform_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor_admin_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    details: serde_json::Value,
) -> ApiResult<()> {
    sqlx::query(
        r#"
        INSERT INTO platform_audit_logs
            (id, actor_admin_id, action, target_type, target_id, result, details)
        VALUES ($1, $2, $3, $4, $5, 'success', $6)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(actor_admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(details)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

fn require_csrf(headers: &HeaderMap, admin: &CurrentPlatformAdmin) -> ApiResult<()> {
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if supplied == Some(admin.csrf_token.as_str()) {
        Ok(())
    } else {
        Err(ApiError::forbidden("CSRFトークンが不正です。"))
    }
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find(|(name, _)| *name == PLATFORM_SESSION_COOKIE)
        .map(|(_, value)| value.to_owned())
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

fn validate_suspension_reason(active: bool, reason: Option<String>) -> ApiResult<Option<String>> {
    if active {
        return Ok(None);
    }
    let reason = reason.unwrap_or_default().trim().to_owned();
    if reason.is_empty() || reason.chars().count() > 500 {
        Err(ApiError::bad_request(
            "INVALID_SUSPENSION_REASON",
            "停止理由は1～500文字で入力してください。",
        ))
    } else {
        Ok(Some(reason))
    }
}

fn invalid_credentials() -> ApiError {
    ApiError::unauthorized_with(
        "INVALID_PLATFORM_CREDENTIALS",
        "メールアドレス、パスワード、または認証コードが正しくありません。",
    )
}

fn verify_totp(secret: &[u8], code: &str) -> bool {
    let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    verify_totp_at(secret, code, elapsed.as_secs())
}

fn verify_totp_at(secret: &[u8], code: &str, timestamp: u64) -> bool {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let Ok(expected) = code.parse::<u32>() else {
        return false;
    };
    let counter = timestamp / TOTP_STEP_SECONDS;
    [counter.saturating_sub(1), counter, counter + 1]
        .into_iter()
        .any(|value| totp_code(secret, value) == expected)
}

fn totp_code(secret: &[u8], counter: u64) -> u32 {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("HMAC accepts keys of any size");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    binary % TOTP_DIGITS
}

#[cfg(test)]
mod tests {
    use super::{totp_code, verify_totp_at};

    #[test]
    fn totp_accepts_current_and_adjacent_steps() {
        let secret = b"12345678901234567890";
        let timestamp = 59;
        let code = format!("{:06}", totp_code(secret, timestamp / 30));

        assert!(verify_totp_at(secret, &code, timestamp));
        assert!(verify_totp_at(secret, &code, timestamp + 30));
        assert!(!verify_totp_at(secret, "invalid", timestamp));
    }
}

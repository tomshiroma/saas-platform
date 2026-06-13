use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    auth::{
        authenticate, hash_password, require_csrf, required_text, set_tenant_context,
        validate_password, write_audit,
    },
    error::{ApiError, ApiResult},
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct TenantResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateTenantRequest {
    pub name: String,
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub display_name: String,
    pub role: String,
    pub active: bool,
}

#[utoipa::path(
    get,
    path = "/api/v1/tenant",
    responses((status = 200, description = "Current tenant", body = TenantResponse))
)]
pub async fn get_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<TenantResponse>> {
    let user = authenticate(&state, &headers).await?;
    Ok(Json(TenantResponse {
        id: user.tenant_id,
        slug: user.tenant_slug,
        name: user.tenant_name,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/tenant",
    request_body = UpdateTenantRequest,
    responses((status = 200, description = "Tenant updated", body = TenantResponse))
)]
pub async fn update_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateTenantRequest>,
) -> ApiResult<Json<TenantResponse>> {
    let user = authenticate(&state, &headers).await?;
    require_admin(&user)?;
    require_csrf(&headers, &user)?;
    let name = required_text(&request.name, "テナント名", 100)?;

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, user.tenant_id).await?;
    sqlx::query("UPDATE tenants SET name = $1, updated_at = now() WHERE id = $2")
        .bind(&name)
        .bind(user.tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    write_audit(
        &mut transaction,
        user.tenant_id,
        user.id,
        "tenant.update",
        "tenant",
        Some(user.tenant_id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    Ok(Json(TenantResponse {
        id: user.tenant_id,
        slug: user.tenant_slug,
        name,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/tenant/users",
    responses((status = 200, description = "Tenant users", body = [UserResponse]))
)]
pub async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<UserResponse>>> {
    let user = authenticate(&state, &headers).await?;
    require_admin(&user)?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, user.tenant_id).await?;
    let users = sqlx::query_as::<_, UserResponse>(
        r#"
        SELECT id, email, display_name, role, active
        FROM users
        WHERE tenant_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(user.tenant_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    transaction.commit().await.map_err(ApiError::internal)?;
    Ok(Json(users))
}

#[utoipa::path(
    post,
    path = "/api/v1/tenant/users",
    request_body = CreateUserRequest,
    responses((status = 201, description = "User created", body = UserResponse))
)]
pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateUserRequest>,
) -> ApiResult<impl IntoResponse> {
    let actor = authenticate(&state, &headers).await?;
    require_admin(&actor)?;
    require_csrf(&headers, &actor)?;
    let email = normalize_email(&request.email)?;
    let display_name = required_text(&request.display_name, "表示名", 100)?;
    let role = validate_role(&request.role)?;
    validate_password(&request.password)?;
    let password_hash = hash_password(request.password).await?;
    let user_id = Uuid::now_v7();

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, actor.tenant_id).await?;
    let result = sqlx::query(
        r#"
        INSERT INTO users (id, tenant_id, email, display_name, role, password_hash)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(user_id)
    .bind(actor.tenant_id)
    .bind(&email)
    .bind(&display_name)
    .bind(&role)
    .bind(password_hash)
    .execute(&mut *transaction)
    .await;

    if let Err(error) = result {
        if is_unique_violation(&error) {
            return Err(ApiError::conflict(
                "USER_EMAIL_EXISTS",
                "このメールアドレスは既に登録されています。",
            ));
        }
        return Err(ApiError::internal(error));
    }

    write_audit(
        &mut transaction,
        actor.tenant_id,
        actor.id,
        "user.create",
        "user",
        Some(user_id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(UserResponse {
            id: user_id,
            email,
            display_name,
            role,
            active: true,
        }),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/tenant/users/{user_id}",
    params(("user_id" = Uuid, Path, description = "User ID")),
    request_body = UpdateUserRequest,
    responses((status = 200, description = "User updated", body = UserResponse))
)]
pub async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdateUserRequest>,
) -> ApiResult<Json<UserResponse>> {
    let actor = authenticate(&state, &headers).await?;
    require_admin(&actor)?;
    require_csrf(&headers, &actor)?;
    let display_name = required_text(&request.display_name, "表示名", 100)?;
    let role = validate_role(&request.role)?;
    if actor.id == user_id && !request.active {
        return Err(ApiError::conflict(
            "CANNOT_DEACTIVATE_SELF",
            "自分自身を無効化できません。",
        ));
    }

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, actor.tenant_id).await?;
    let existing = sqlx::query_as::<_, UserResponse>(
        "SELECT id, email, display_name, role, active FROM users WHERE id = $1 AND tenant_id = $2",
    )
    .bind(user_id)
    .bind(actor.tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("ユーザーが見つかりません。"))?;

    if existing.role == "admin" && (role != "admin" || !request.active) {
        ensure_another_admin(&mut transaction, actor.tenant_id, user_id).await?;
    }

    let updated = sqlx::query_as::<_, UserResponse>(
        r#"
        UPDATE users
        SET display_name = $1, role = $2, active = $3, updated_at = now()
        WHERE id = $4 AND tenant_id = $5
        RETURNING id, email, display_name, role, active
        "#,
    )
    .bind(display_name)
    .bind(role)
    .bind(request.active)
    .bind(user_id)
    .bind(actor.tenant_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;

    if !updated.active {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
    }
    write_audit(
        &mut transaction,
        actor.tenant_id,
        actor.id,
        "user.update",
        "user",
        Some(user_id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;
    Ok(Json(updated))
}

#[utoipa::path(
    delete,
    path = "/api/v1/tenant/users/{user_id}",
    params(("user_id" = Uuid, Path, description = "User ID")),
    responses((status = 204, description = "User deleted"))
)]
pub async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> ApiResult<axum::http::StatusCode> {
    let actor = authenticate(&state, &headers).await?;
    require_admin(&actor)?;
    require_csrf(&headers, &actor)?;
    if actor.id == user_id {
        return Err(ApiError::conflict(
            "CANNOT_DELETE_SELF",
            "自分自身は削除できません。",
        ));
    }

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, actor.tenant_id).await?;
    let existing =
        sqlx::query_as::<_, (String,)>("SELECT role FROM users WHERE id = $1 AND tenant_id = $2")
            .bind(user_id)
            .bind(actor.tenant_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::not_found("ユーザーが見つかりません。"))?;

    if existing.0 == "admin" {
        ensure_another_admin(&mut transaction, actor.tenant_id, user_id).await?;
    }

    sqlx::query("DELETE FROM users WHERE id = $1 AND tenant_id = $2")
        .bind(user_id)
        .bind(actor.tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    write_audit(
        &mut transaction,
        actor.tenant_id,
        actor.id,
        "user.delete",
        "user",
        Some(user_id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn ensure_another_admin(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    excluded_user_id: Uuid,
) -> ApiResult<()> {
    let admin_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(*)
        FROM users
        WHERE tenant_id = $1 AND id <> $2 AND role = 'admin' AND active = true
        "#,
    )
    .bind(tenant_id)
    .bind(excluded_user_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;

    if admin_count == 0 {
        Err(ApiError::conflict(
            "LAST_ADMIN_REQUIRED",
            "テナントには有効な管理者が1人以上必要です。",
        ))
    } else {
        Ok(())
    }
}

fn require_admin(user: &crate::auth::CurrentUser) -> ApiResult<()> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(ApiError::forbidden("管理者権限が必要です。"))
    }
}

fn validate_role(role: &str) -> ApiResult<String> {
    match role {
        "admin" | "member" => Ok(role.to_owned()),
        _ => Err(ApiError::bad_request(
            "INVALID_ROLE",
            "権限はadminまたはmemberを指定してください。",
        )),
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

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(database_error)
            if database_error.code().as_deref() == Some("23505")
    )
}

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderName, Request},
    middleware::{self, Next},
    response::Response,
    routing::{get, patch, post},
};
use serde::Serialize;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use utoipa::{OpenApi, ToSchema};

use crate::{
    auth::{self, AuthResponse, LoginRequest, RegisterRequest},
    state::AppState,
    tenant::{
        self, CreateUserRequest, TenantResponse, UpdateTenantRequest, UpdateUserRequest,
        UserResponse,
    },
};

#[derive(Serialize, ToSchema)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        live,
        ready,
        auth::register,
        auth::login,
        auth::me,
        auth::logout,
        tenant::get_tenant,
        tenant::update_tenant,
        tenant::list_users,
        tenant::create_user,
        tenant::update_user,
        tenant::delete_user
    ),
    components(schemas(
        HealthResponse,
        RegisterRequest,
        LoginRequest,
        AuthResponse,
        TenantResponse,
        UpdateTenantRequest,
        UserResponse,
        CreateUserRequest,
        UpdateUserRequest
    ))
)]
struct ApiDoc;

pub fn router(state: AppState) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");

    Router::new()
        .route("/api/v1/health/live", get(live))
        .route("/api/v1/health/ready", get(ready))
        .route("/api/v1/openapi.json", get(openapi))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/me", get(auth::me))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route(
            "/api/v1/tenant",
            get(tenant::get_tenant).patch(tenant::update_tenant),
        )
        .route(
            "/api/v1/tenant/users",
            get(tenant::list_users).post(tenant::create_user),
        )
        .route(
            "/api/v1/tenant/users/{user_id}",
            patch(tenant::update_user).delete(tenant::delete_user),
        )
        .with_state(state)
        .layer(middleware::from_fn(log_request_id))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
}

#[utoipa::path(
    get,
    path = "/api/v1/health/live",
    responses((status = 200, description = "Process is alive", body = HealthResponse))
)]
async fn live() -> Json<HealthResponse> {
    Json(health_response())
}

#[utoipa::path(
    get,
    path = "/api/v1/health/ready",
    responses(
        (status = 200, description = "Service is ready", body = HealthResponse),
        (status = 503, description = "Database is unavailable")
    )
)]
async fn ready(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, axum::http::StatusCode> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(health_response()))
}
async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

async fn log_request_id(request: Request<axum::body::Body>, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("missing")
        .to_owned();
    let method = request.method().clone();
    let uri = request.uri().clone();

    let response = next.run(request).await;
    tracing::info!(%request_id, %method, %uri, status = %response.status(), "request completed");
    response
}

fn health_response() -> HealthResponse {
    HealthResponse {
        status: "ok",
        service: "saas-api",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::health_response;

    #[test]
    fn health_response_contains_service_metadata() {
        let response = health_response();

        assert_eq!(response.status, "ok");
        assert_eq!(response.service, "saas-api");
        assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
    }
}

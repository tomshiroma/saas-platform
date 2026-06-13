use axum::{
    Json, Router,
    extract::State,
    http::{HeaderName, Request},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use serde::Serialize;
use sqlx::PgPool;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use utoipa::{OpenApi, ToSchema};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[derive(Serialize, ToSchema)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(OpenApi)]
#[openapi(paths(live, ready), components(schemas(HealthResponse)))]
struct ApiDoc;

pub fn router(pool: PgPool) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");
    let state = AppState { pool };

    Router::new()
        .route("/api/v1/health/live", get(live))
        .route("/api/v1/health/ready", get(ready))
        .route("/api/v1/openapi.json", get(openapi))
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

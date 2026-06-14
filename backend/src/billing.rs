use std::time::{SystemTime, UNIX_EPOCH};

use axum::{Json, body::Bytes, extract::State, http::HeaderMap};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    auth::{authenticate, require_csrf, set_tenant_context},
    error::{ApiError, ApiResult},
    platform::BillingPlanResponse,
    state::AppState,
};

const WEBHOOK_TOLERANCE_SECONDS: u64 = 300;

#[derive(Serialize, ToSchema)]
pub struct BillingStatusResponse {
    pub plan: Option<BillingPlanResponse>,
    pub status: Option<String>,
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: bool,
    pub unit_amount: Option<i64>,
    pub billing_interval: Option<String>,
    pub stripe_configured: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateCheckoutRequest {
    pub plan_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct StripeRedirectResponse {
    pub url: String,
}

#[derive(Deserialize)]
struct StripeEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    data: StripeEventData,
}

#[derive(Deserialize)]
struct StripeEventData {
    object: Value,
}

#[utoipa::path(
    get,
    path = "/api/v1/billing/plans",
    responses((status = 200, description = "Available billing plans", body = [BillingPlanResponse]))
)]
pub async fn list_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<BillingPlanResponse>>> {
    authenticate(&state, &headers).await?;
    let plans = sqlx::query_as::<_, BillingPlanResponse>(
        r#"
        SELECT
            id, code, name, description, currency, unit_amount,
            billing_interval, active, stripe_product_id, stripe_price_id
        FROM billing_plans
        WHERE active = true
        ORDER BY unit_amount ASC, created_at ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    Ok(Json(plans))
}

#[utoipa::path(
    get,
    path = "/api/v1/billing/status",
    responses((status = 200, description = "Current subscription status", body = BillingStatusResponse))
)]
pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<BillingStatusResponse>> {
    let user = authenticate(&state, &headers).await?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, user.tenant_id).await?;
    let subscription = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            bool,
            Option<i64>,
            Option<String>,
        ),
    >(
        r#"
        SELECT
            plan_id,
            status,
            CASE
                WHEN current_period_end IS NULL THEN NULL
                ELSE to_char(
                    current_period_end AT TIME ZONE 'UTC',
                    'YYYY-MM-DD"T"HH24:MI:SS"Z"'
                )
            END,
            cancel_at_period_end,
            unit_amount,
            billing_interval
        FROM tenant_subscriptions
        WHERE tenant_id = $1
        "#,
    )
    .bind(user.tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let (plan, status, current_period_end, cancel_at_period_end, unit_amount, billing_interval) =
        if let Some(subscription) = subscription {
            let plan = sqlx::query_as::<_, BillingPlanResponse>(
                r#"
                SELECT
                    id, code, name, description, currency, unit_amount,
                    billing_interval, active, stripe_product_id, stripe_price_id
                FROM billing_plans
                WHERE id = $1
                "#,
            )
            .bind(subscription.0)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;
            (
                plan,
                Some(subscription.1),
                subscription.2,
                subscription.3,
                subscription.4,
                subscription.5,
            )
        } else {
            (None, None, None, false, None, None)
        };

    Ok(Json(BillingStatusResponse {
        plan,
        status,
        current_period_end,
        cancel_at_period_end,
        unit_amount,
        billing_interval,
        stripe_configured: state.stripe.is_configured(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/billing/checkout",
    request_body = CreateCheckoutRequest,
    responses((status = 200, description = "Stripe Checkout URL", body = StripeRedirectResponse))
)]
pub async fn create_checkout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateCheckoutRequest>,
) -> ApiResult<Json<StripeRedirectResponse>> {
    let user = authenticate(&state, &headers).await?;
    require_tenant_admin(&user)?;
    require_csrf(&headers, &user)?;
    ensure_stripe_configured(&state)?;
    let plan = sqlx::query_as::<_, BillingPlanResponse>(
        r#"
        SELECT
            id, code, name, description, currency, unit_amount,
            billing_interval, active, stripe_product_id, stripe_price_id
        FROM billing_plans
        WHERE id = $1 AND active = true
        "#,
    )
    .bind(request.plan_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("利用可能なプランが見つかりません。"))?;

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, user.tenant_id).await?;
    let existing = sqlx::query_as::<_, (String, Option<String>, String)>(
        r#"
        SELECT stripe_customer_id, stripe_subscription_id, status
        FROM tenant_subscriptions
        WHERE tenant_id = $1
        "#,
    )
    .bind(user.tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    transaction.commit().await.map_err(ApiError::internal)?;

    if existing
        .as_ref()
        .is_some_and(|value| value.1.is_some() && value.2 != "canceled")
    {
        return Err(ApiError::conflict(
            "SUBSCRIPTION_ALREADY_EXISTS",
            "既存契約の変更はStripe請求ポータルから行ってください。",
        ));
    }

    let customer_id = if let Some(existing) = existing {
        existing.0
    } else {
        state
            .stripe
            .create_customer(&user.email, &user.tenant_name, user.tenant_id)
            .await
            .map_err(stripe_error)?
            .id
    };

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, user.tenant_id).await?;
    sqlx::query(
        r#"
        INSERT INTO tenant_subscriptions
            (id, tenant_id, plan_id, stripe_customer_id, status,
             stripe_price_id, unit_amount, billing_interval)
        VALUES ($1, $2, $3, $4, 'checkout_pending', $5, $6, $7)
        ON CONFLICT (tenant_id)
        DO UPDATE SET
            plan_id = EXCLUDED.plan_id,
            stripe_customer_id = EXCLUDED.stripe_customer_id,
            stripe_subscription_id = NULL,
            status = 'checkout_pending',
            stripe_price_id = EXCLUDED.stripe_price_id,
            unit_amount = EXCLUDED.unit_amount,
            billing_interval = EXCLUDED.billing_interval,
            updated_at = now()
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user.tenant_id)
    .bind(plan.id)
    .bind(&customer_id)
    .bind(&plan.stripe_price_id)
    .bind(plan.unit_amount)
    .bind(&plan.billing_interval)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let session = state
        .stripe
        .create_checkout_session(
            &customer_id,
            &plan.stripe_price_id,
            user.tenant_id,
            plan.id,
            &format!("{}/billing?checkout=success", state.app_base_url),
            &format!("{}/billing?checkout=cancel", state.app_base_url),
        )
        .await
        .map_err(stripe_error)?;
    tracing::info!(
        checkout_session_id = %session.id,
        tenant_id = %user.tenant_id,
        plan_id = %plan.id,
        "Stripe Checkout session created"
    );
    Ok(Json(StripeRedirectResponse { url: session.url }))
}

#[utoipa::path(
    post,
    path = "/api/v1/billing/portal",
    responses((status = 200, description = "Stripe Customer Portal URL", body = StripeRedirectResponse))
)]
pub async fn create_portal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<StripeRedirectResponse>> {
    let user = authenticate(&state, &headers).await?;
    require_tenant_admin(&user)?;
    require_csrf(&headers, &user)?;
    ensure_stripe_configured(&state)?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    set_tenant_context(&mut transaction, user.tenant_id).await?;
    let customer_id = sqlx::query_scalar::<_, String>(
        "SELECT stripe_customer_id FROM tenant_subscriptions WHERE tenant_id = $1",
    )
    .bind(user.tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("Stripe契約情報がありません。"))?;
    transaction.commit().await.map_err(ApiError::internal)?;

    let session = state
        .stripe
        .create_portal_session(&customer_id, &format!("{}/billing", state.app_base_url))
        .await
        .map_err(stripe_error)?;
    Ok(Json(StripeRedirectResponse { url: session.url }))
}

pub async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<axum::http::StatusCode> {
    let signature = headers
        .get("stripe-signature")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(invalid_webhook)?;
    verify_webhook_signature(
        state.stripe.webhook_secret().map_err(stripe_error)?,
        signature,
        &body,
    )?;
    let event: StripeEvent = serde_json::from_slice(&body).map_err(|_| invalid_webhook())?;
    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    let inserted = sqlx::query(
        r#"
        INSERT INTO stripe_webhook_events (id, event_type)
        VALUES ($1, $2)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&event.id)
    .bind(&event.event_type)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    if inserted.rows_affected() == 0 {
        return Ok(axum::http::StatusCode::OK);
    }

    match event.event_type.as_str() {
        "checkout.session.completed" => {
            apply_checkout_completed(&mut transaction, &event.data.object).await?;
        }
        "customer.subscription.created" | "customer.subscription.updated" => {
            apply_subscription_updated(&mut transaction, &event.data.object).await?;
        }
        "customer.subscription.deleted" => {
            apply_subscription_deleted(&mut transaction, &event.data.object).await?;
        }
        _ => {}
    }
    transaction.commit().await.map_err(ApiError::internal)?;
    Ok(axum::http::StatusCode::OK)
}

#[allow(dead_code)]
#[utoipa::path(
    post,
    path = "/api/v1/stripe/webhook",
    request_body(content = Value, content_type = "application/json"),
    responses((status = 200, description = "Webhook processed"))
)]
pub async fn stripe_webhook_openapi(Json(_body): Json<Value>) {}

async fn apply_checkout_completed(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    object: &Value,
) -> ApiResult<()> {
    let tenant_id = metadata_uuid(object, "tenant_id")?;
    let plan_id = metadata_uuid(object, "plan_id")?;
    let customer_id = required_string(object, "/customer")?;
    let subscription_id = required_string(object, "/subscription")?;
    set_tenant_context(transaction, tenant_id).await?;
    sqlx::query(
        r#"
        UPDATE tenant_subscriptions
        SET
            plan_id = $1,
            stripe_customer_id = $2,
            stripe_subscription_id = $3,
            status = 'active',
            updated_at = now()
        WHERE tenant_id = $4
        "#,
    )
    .bind(plan_id)
    .bind(customer_id)
    .bind(subscription_id)
    .bind(tenant_id)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

async fn apply_subscription_updated(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    object: &Value,
) -> ApiResult<()> {
    let tenant_id = metadata_uuid(object, "tenant_id")?;
    let subscription_id = required_string(object, "/id")?;
    let customer_id = required_string(object, "/customer")?;
    let status = required_string(object, "/status")?;
    let stripe_price_id = required_string(object, "/items/data/0/price/id")?;
    let plan_id =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM billing_plans WHERE stripe_price_id = $1")
            .bind(&stripe_price_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(ApiError::internal)?
            .or_else(|| metadata_uuid(object, "plan_id").ok())
            .ok_or_else(invalid_webhook)?;
    let unit_amount = object
        .pointer("/items/data/0/price/unit_amount")
        .and_then(Value::as_i64);
    let billing_interval = object
        .pointer("/items/data/0/price/recurring/interval")
        .and_then(Value::as_str);
    let period_end = object
        .pointer("/current_period_end")
        .and_then(Value::as_i64)
        .or_else(|| {
            object
                .pointer("/items/data/0/current_period_end")
                .and_then(Value::as_i64)
        });
    let cancel_at_period_end = object
        .pointer("/cancel_at_period_end")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    set_tenant_context(transaction, tenant_id).await?;
    sqlx::query(
        r#"
        INSERT INTO tenant_subscriptions
            (id, tenant_id, plan_id, stripe_customer_id,
             stripe_subscription_id, status, current_period_end,
             cancel_at_period_end, stripe_price_id, unit_amount,
             billing_interval)
        VALUES (
            $1, $2, $3, $4, $5, $6,
            CASE WHEN $7::bigint IS NULL THEN NULL ELSE to_timestamp($7) END,
            $8, $9, $10, $11
        )
        ON CONFLICT (tenant_id)
        DO UPDATE SET
            plan_id = EXCLUDED.plan_id,
            stripe_customer_id = EXCLUDED.stripe_customer_id,
            stripe_subscription_id = EXCLUDED.stripe_subscription_id,
            status = EXCLUDED.status,
            current_period_end = EXCLUDED.current_period_end,
            cancel_at_period_end = EXCLUDED.cancel_at_period_end,
            stripe_price_id = EXCLUDED.stripe_price_id,
            unit_amount = EXCLUDED.unit_amount,
            billing_interval = EXCLUDED.billing_interval,
            updated_at = now()
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(plan_id)
    .bind(customer_id)
    .bind(subscription_id)
    .bind(status)
    .bind(period_end)
    .bind(cancel_at_period_end)
    .bind(stripe_price_id)
    .bind(unit_amount)
    .bind(billing_interval)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

async fn apply_subscription_deleted(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    object: &Value,
) -> ApiResult<()> {
    let tenant_id = metadata_uuid(object, "tenant_id")?;
    let subscription_id = required_string(object, "/id")?;
    set_tenant_context(transaction, tenant_id).await?;
    sqlx::query(
        r#"
        UPDATE tenant_subscriptions
        SET
            status = 'canceled',
            cancel_at_period_end = false,
            updated_at = now()
        WHERE tenant_id = $1 AND stripe_subscription_id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(subscription_id)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

fn verify_webhook_signature(secret: &str, signature: &str, body: &[u8]) -> ApiResult<()> {
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in signature.split(',') {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "t" => timestamp = value.parse::<u64>().ok(),
                "v1" => signatures.push(value),
                _ => {}
            }
        }
    }
    let timestamp = timestamp.ok_or_else(invalid_webhook)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(ApiError::internal)?
        .as_secs();
    if now.abs_diff(timestamp) > WEBHOOK_TOLERANCE_SECONDS {
        return Err(invalid_webhook());
    }
    let signed_payload = [timestamp.to_string().as_bytes(), b".", body].concat();
    let valid = signatures.into_iter().any(|signature| {
        decode_hex(signature).is_some_and(|expected| {
            let mut mac =
                Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key");
            mac.update(&signed_payload);
            mac.verify_slice(&expected).is_ok()
        })
    });
    if valid {
        Ok(())
    } else {
        Err(invalid_webhook())
    }
}

fn metadata_uuid(object: &Value, name: &str) -> ApiResult<Uuid> {
    object
        .pointer(&format!("/metadata/{name}"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_webhook)
}

fn required_string(object: &Value, pointer: &str) -> ApiResult<String> {
    object
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(invalid_webhook)
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|text| u8::from_str_radix(text, 16).ok())
        })
        .collect()
}

fn require_tenant_admin(user: &crate::auth::CurrentUser) -> ApiResult<()> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(ApiError::forbidden("契約管理には管理者権限が必要です。"))
    }
}

fn ensure_stripe_configured(state: &AppState) -> ApiResult<()> {
    if state.stripe.is_configured() {
        Ok(())
    } else {
        Err(ApiError::service_unavailable(
            "STRIPE_NOT_CONFIGURED",
            "Stripeが設定されていません。",
        ))
    }
}

fn stripe_error(error: anyhow::Error) -> ApiError {
    tracing::error!(%error, "Stripe operation failed");
    ApiError::service_unavailable(
        "STRIPE_UNAVAILABLE",
        "Stripeとの連携に失敗しました。時間をおいて再試行してください。",
    )
}

fn invalid_webhook() -> ApiError {
    ApiError::bad_request(
        "INVALID_STRIPE_WEBHOOK",
        "Stripe Webhookの検証に失敗しました。",
    )
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    use super::{decode_hex, verify_webhook_signature};

    #[test]
    fn webhook_signature_is_verified_against_raw_body() {
        let secret = "whsec_test";
        let body = br#"{"id":"evt_test"}"#;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let payload = [timestamp.to_string().as_bytes(), b".", body].concat();
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(&payload);
        let signature = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        assert!(
            verify_webhook_signature(secret, &format!("t={timestamp},v1={signature}"), body)
                .is_ok()
        );
        assert!(verify_webhook_signature(secret, &format!("t={timestamp},v1=00"), body).is_err());
        assert_eq!(decode_hex("00ff"), Some(vec![0, 255]));
    }
}

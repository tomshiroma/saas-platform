use anyhow::{Context, Result, bail};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::config::Config;

const STRIPE_API_VERSION: &str = "2026-05-27.dahlia";

#[derive(Clone)]
pub struct StripeClient {
    client: Client,
    secret_key: String,
    webhook_secret: String,
    api_base_url: String,
}

#[derive(Deserialize)]
pub struct StripeObject {
    pub id: String,
}

#[derive(Deserialize)]
pub struct StripeCheckoutSession {
    pub id: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct StripePortalSession {
    pub url: String,
}

impl StripeClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            secret_key: config.stripe_secret_key.clone(),
            webhook_secret: config.stripe_webhook_secret.clone(),
            api_base_url: config.stripe_api_base_url.trim_end_matches('/').to_owned(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.secret_key.is_empty()
    }

    pub fn webhook_secret(&self) -> Result<&str> {
        if self.webhook_secret.is_empty() {
            bail!("STRIPE_WEBHOOK_SECRET is not configured");
        }
        Ok(&self.webhook_secret)
    }

    pub async fn create_product(
        &self,
        name: &str,
        description: &str,
        plan_id: Uuid,
    ) -> Result<StripeObject> {
        self.post(
            "/v1/products",
            vec![
                ("name".to_owned(), name.to_owned()),
                ("description".to_owned(), description.to_owned()),
                ("metadata[plan_id]".to_owned(), plan_id.to_string()),
            ],
            Some(format!("plan-product-{plan_id}")),
        )
        .await
    }

    pub async fn update_product(
        &self,
        product_id: &str,
        name: &str,
        description: &str,
        active: bool,
    ) -> Result<StripeObject> {
        self.post(
            &format!("/v1/products/{product_id}"),
            vec![
                ("name".to_owned(), name.to_owned()),
                ("description".to_owned(), description.to_owned()),
                ("active".to_owned(), active.to_string()),
            ],
            None,
        )
        .await
    }

    pub async fn create_price(
        &self,
        product_id: &str,
        unit_amount: i64,
        interval: &str,
        plan_id: Uuid,
    ) -> Result<StripeObject> {
        self.post(
            "/v1/prices",
            vec![
                ("product".to_owned(), product_id.to_owned()),
                ("currency".to_owned(), "jpy".to_owned()),
                ("unit_amount".to_owned(), unit_amount.to_string()),
                ("recurring[interval]".to_owned(), interval.to_owned()),
                ("metadata[plan_id]".to_owned(), plan_id.to_string()),
            ],
            Some(format!("plan-price-{plan_id}-{unit_amount}-{interval}")),
        )
        .await
    }

    pub async fn set_price_active(&self, price_id: &str, active: bool) -> Result<StripeObject> {
        self.post(
            &format!("/v1/prices/{price_id}"),
            vec![("active".to_owned(), active.to_string())],
            None,
        )
        .await
    }

    pub async fn create_customer(
        &self,
        email: &str,
        tenant_name: &str,
        tenant_id: Uuid,
    ) -> Result<StripeObject> {
        self.post(
            "/v1/customers",
            vec![
                ("email".to_owned(), email.to_owned()),
                ("name".to_owned(), tenant_name.to_owned()),
                ("metadata[tenant_id]".to_owned(), tenant_id.to_string()),
            ],
            Some(format!("tenant-customer-{tenant_id}")),
        )
        .await
    }

    pub async fn create_checkout_session(
        &self,
        customer_id: &str,
        price_id: &str,
        tenant_id: Uuid,
        plan_id: Uuid,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<StripeCheckoutSession> {
        self.post(
            "/v1/checkout/sessions",
            vec![
                ("mode".to_owned(), "subscription".to_owned()),
                ("customer".to_owned(), customer_id.to_owned()),
                ("line_items[0][price]".to_owned(), price_id.to_owned()),
                ("line_items[0][quantity]".to_owned(), "1".to_owned()),
                ("success_url".to_owned(), success_url.to_owned()),
                ("cancel_url".to_owned(), cancel_url.to_owned()),
                ("metadata[tenant_id]".to_owned(), tenant_id.to_string()),
                ("metadata[plan_id]".to_owned(), plan_id.to_string()),
                (
                    "subscription_data[metadata][tenant_id]".to_owned(),
                    tenant_id.to_string(),
                ),
                (
                    "subscription_data[metadata][plan_id]".to_owned(),
                    plan_id.to_string(),
                ),
            ],
            Some(format!("checkout-{tenant_id}-{plan_id}-{}", Uuid::now_v7())),
        )
        .await
    }

    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<StripePortalSession> {
        self.post(
            "/v1/billing_portal/sessions",
            vec![
                ("customer".to_owned(), customer_id.to_owned()),
                ("return_url".to_owned(), return_url.to_owned()),
            ],
            None,
        )
        .await
    }

    async fn post<T>(
        &self,
        path: &str,
        form: Vec<(String, String)>,
        idempotency_key: Option<String>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        if !self.is_configured() {
            bail!("STRIPE_SECRET_KEY is not configured");
        }

        let mut request = self
            .client
            .post(format!("{}{path}", self.api_base_url))
            .bearer_auth(&self.secret_key)
            .header("Stripe-Version", STRIPE_API_VERSION)
            .form(&form);
        if let Some(key) = idempotency_key {
            request = request.header("Idempotency-Key", key);
        }

        let response = request.send().await.context("Stripe request failed")?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read Stripe response")?;
        if status != StatusCode::OK {
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|value| {
                    value
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| format!("Stripe returned HTTP {status}"));
            bail!("{message}");
        }
        serde_json::from_str(&body).context("failed to decode Stripe response")
    }
}

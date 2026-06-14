CREATE TABLE billing_plans (
    id uuid PRIMARY KEY,
    code text NOT NULL UNIQUE,
    name text NOT NULL,
    description text NOT NULL DEFAULT '',
    currency text NOT NULL DEFAULT 'jpy'
        CHECK (currency = 'jpy'),
    unit_amount bigint NOT NULL
        CHECK (unit_amount > 0),
    billing_interval text NOT NULL
        CHECK (billing_interval IN ('month', 'year')),
    active boolean NOT NULL DEFAULT true,
    stripe_product_id text NOT NULL UNIQUE,
    stripe_price_id text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE tenant_subscriptions (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL UNIQUE REFERENCES tenants (id) ON DELETE CASCADE,
    plan_id uuid NOT NULL REFERENCES billing_plans (id),
    stripe_customer_id text NOT NULL UNIQUE,
    stripe_subscription_id text UNIQUE,
    status text NOT NULL,
    current_period_end timestamptz,
    cancel_at_period_end boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE tenant_subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenant_subscriptions FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_subscriptions_tenant_isolation ON tenant_subscriptions
    USING (
        tenant_id = NULLIF(
            current_setting('app.current_tenant_id', true),
            ''
        )::uuid
    )
    WITH CHECK (
        tenant_id = NULLIF(
            current_setting('app.current_tenant_id', true),
            ''
        )::uuid
    );

CREATE TABLE stripe_webhook_events (
    id text PRIMARY KEY,
    event_type text NOT NULL,
    processed_at timestamptz NOT NULL DEFAULT now()
);

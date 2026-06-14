ALTER TABLE tenant_subscriptions
    ADD COLUMN stripe_price_id text,
    ADD COLUMN unit_amount bigint
        CHECK (unit_amount IS NULL OR unit_amount > 0),
    ADD COLUMN billing_interval text
        CHECK (
            billing_interval IS NULL
            OR billing_interval IN ('month', 'year')
        );

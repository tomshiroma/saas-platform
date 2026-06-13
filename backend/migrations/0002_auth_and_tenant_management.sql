ALTER TABLE tenants
    ADD COLUMN slug text,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();

UPDATE tenants
SET slug = 'tenant-' || left(id::text, 8)
WHERE slug IS NULL;

ALTER TABLE tenants
    ALTER COLUMN slug SET NOT NULL,
    ADD CONSTRAINT tenants_slug_format CHECK (slug ~ '^[a-z0-9][a-z0-9-]{2,48}[a-z0-9]$'),
    ADD CONSTRAINT tenants_slug_unique UNIQUE (slug);

ALTER TABLE users
    ADD COLUMN password_hash text NOT NULL DEFAULT '$argon2id$v=19$m=19456,t=2,p=1$invalidinvalidinvalid$invalidinvalidinvalidinvalidinvalidinvalidinvalid',
    ADD COLUMN active boolean NOT NULL DEFAULT true,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();

ALTER TABLE users
    ALTER COLUMN password_hash DROP DEFAULT;

CREATE UNIQUE INDEX users_tenant_email_lower_unique
    ON users (tenant_id, lower(email));

ALTER TABLE users DROP CONSTRAINT users_tenant_id_email_key;

CREATE TABLE sessions (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    csrf_token text NOT NULL,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX sessions_user_id_index ON sessions (user_id);
CREATE INDEX sessions_expires_at_index ON sessions (expires_at);

CREATE TABLE audit_logs (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    actor_user_id uuid REFERENCES users (id) ON DELETE SET NULL,
    action text NOT NULL,
    target_type text NOT NULL,
    target_id uuid,
    result text NOT NULL,
    details jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX audit_logs_tenant_created_at_index
    ON audit_logs (tenant_id, created_at DESC);

ALTER TABLE audit_logs ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_logs FORCE ROW LEVEL SECURITY;

CREATE POLICY audit_logs_tenant_isolation ON audit_logs
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

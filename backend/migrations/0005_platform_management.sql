ALTER TABLE tenants
    ADD COLUMN active boolean NOT NULL DEFAULT true,
    ADD COLUMN suspended_at timestamptz,
    ADD COLUMN suspension_reason text;

CREATE TABLE platform_admins (
    id uuid PRIMARY KEY,
    email text NOT NULL,
    display_name text NOT NULL,
    password_hash text NOT NULL,
    totp_secret bytea NOT NULL,
    active boolean NOT NULL DEFAULT true,
    failed_login_count integer NOT NULL DEFAULT 0
        CHECK (failed_login_count >= 0),
    locked_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX platform_admins_email_lower_unique
    ON platform_admins (lower(email));

CREATE TABLE platform_sessions (
    id uuid PRIMARY KEY,
    admin_id uuid NOT NULL REFERENCES platform_admins (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    csrf_token text NOT NULL,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX platform_sessions_admin_id_index
    ON platform_sessions (admin_id);

CREATE INDEX platform_sessions_expires_at_index
    ON platform_sessions (expires_at);

CREATE TABLE platform_audit_logs (
    id uuid PRIMARY KEY,
    actor_admin_id uuid REFERENCES platform_admins (id) ON DELETE SET NULL,
    action text NOT NULL,
    target_type text NOT NULL,
    target_id uuid,
    result text NOT NULL,
    details jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX platform_audit_logs_created_at_index
    ON platform_audit_logs (created_at DESC);

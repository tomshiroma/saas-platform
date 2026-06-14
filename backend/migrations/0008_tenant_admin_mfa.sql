ALTER TABLE users
    ADD COLUMN mfa_secret_encrypted bytea,
    ADD COLUMN mfa_enabled_at timestamptz;

ALTER TABLE sessions
    ADD COLUMN mfa_verified boolean NOT NULL DEFAULT false;

CREATE TABLE user_mfa_recovery_codes (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_hash bytea NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    used_at timestamptz,
    UNIQUE (user_id, code_hash)
);

CREATE INDEX user_mfa_recovery_codes_user_id_index
    ON user_mfa_recovery_codes (user_id)
    WHERE used_at IS NULL;

ALTER TABLE user_mfa_recovery_codes ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_mfa_recovery_codes FORCE ROW LEVEL SECURITY;

CREATE POLICY user_mfa_recovery_codes_tenant_isolation
    ON user_mfa_recovery_codes
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

CREATE TABLE user_mfa_challenges (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    purpose text NOT NULL CHECK (purpose IN ('setup', 'verify')),
    secret_encrypted bytea,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX user_mfa_challenges_expires_at_index
    ON user_mfa_challenges (expires_at);

ALTER TABLE user_mfa_challenges ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_mfa_challenges FORCE ROW LEVEL SECURITY;

CREATE POLICY user_mfa_challenges_tenant_isolation
    ON user_mfa_challenges
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

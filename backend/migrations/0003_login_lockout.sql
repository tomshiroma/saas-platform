ALTER TABLE users
    ADD COLUMN failed_login_count integer NOT NULL DEFAULT 0
        CHECK (failed_login_count >= 0),
    ADD COLUMN locked_until timestamptz;


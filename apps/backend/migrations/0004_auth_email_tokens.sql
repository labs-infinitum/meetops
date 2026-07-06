-- Email verification and password reset tokens.

ALTER TABLE users ADD COLUMN email_verified_at BIGINT;

-- Existing accounts are treated as already verified.
UPDATE users SET email_verified_at = created_at WHERE email_verified_at IS NULL;

CREATE TABLE auth_email_tokens (
    token TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    purpose TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX idx_auth_email_tokens_user_purpose ON auth_email_tokens(user_id, purpose);
CREATE INDEX idx_auth_email_tokens_expires_at ON auth_email_tokens(expires_at);

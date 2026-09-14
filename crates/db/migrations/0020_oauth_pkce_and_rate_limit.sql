-- PKCE (RFC 7636) columns for authorization codes.
ALTER TABLE oauth_authorization_codes
  ADD COLUMN IF NOT EXISTS code_challenge TEXT,
  ADD COLUMN IF NOT EXISTS code_challenge_method TEXT;

-- Shared rate-limit buckets across api-server replicas (auth + faucet classes).
CREATE TABLE IF NOT EXISTS rate_limit_buckets (
  bucket_key TEXT PRIMARY KEY,
  window_start TIMESTAMPTZ NOT NULL,
  hits INT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_rate_limit_buckets_window
  ON rate_limit_buckets (window_start);

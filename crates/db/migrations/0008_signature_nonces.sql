-- One-shot nonce store for HMAC-signed public API requests. A row per
-- (api_key_id, signature) reserved atomically via INSERT ... ON CONFLICT DO
-- NOTHING — the same signature can never be replayed inside the timestamp
-- window. Postgres, not Redis, for the same reason `captcha_seen_tokens`
-- is Postgres: fail-closed consistency with the rest of the codebase
-- without adding a second store just for an anti-replay cache.
CREATE TABLE public_api_signature_nonces (
    api_key_id  uuid NOT NULL,
    signature   text NOT NULL,
    seen_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (api_key_id, signature)
);

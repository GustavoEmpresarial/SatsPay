-- Worker-to-API custody handshake (ADR 0012). The singleton is refreshed by
-- the live signer only after its private keys match the public xpub/address
-- configuration. The API also checks freshness, so a dead signer fails closed.
CREATE TABLE IF NOT EXISTS custody_signer_validation (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    config_fingerprint VARCHAR(64) NOT NULL,
    worker_version TEXT NOT NULL,
    validated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

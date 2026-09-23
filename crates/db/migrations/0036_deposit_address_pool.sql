-- ADR 0012: the api-server holds no signing keys. Coins without public
-- derivation (SOL / ed25519) get deposit addresses pre-derived by the worker,
-- which holds DEPOSIT_MNEMONIC. The api-server claims one row per new
-- address with `FOR UPDATE SKIP LOCKED`; `hd_index` comes from the same
-- per-coin sequence, so the worker can later derive the key to sweep it.
CREATE TABLE IF NOT EXISTS deposit_address_pool (
    coin coin NOT NULL,
    hd_index BIGINT NOT NULL CHECK (hd_index >= 0),
    address TEXT NOT NULL UNIQUE CHECK (char_length(address) BETWEEN 8 AND 256),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Set once, never cleared: a claimed address is never handed out again,
    -- even if the claiming transaction rolled back.
    claimed_at TIMESTAMPTZ,
    PRIMARY KEY (coin, hd_index)
);

CREATE INDEX IF NOT EXISTS idx_deposit_address_pool_unclaimed
    ON deposit_address_pool (coin, hd_index)
    WHERE claimed_at IS NULL;

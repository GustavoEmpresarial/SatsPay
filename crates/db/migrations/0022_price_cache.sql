-- Restores `price_cache` for fresh environments. The original `0007_price_cache.sql`
-- is missing from the tree (tolerated via migrator.set_ignore_missing on prod DBs
-- that already applied it). Idempotent for those databases.

CREATE TABLE IF NOT EXISTS price_cache (
    coin           coin PRIMARY KEY,
    price_scaled   NUMERIC NOT NULL,
    price_decimals INT NOT NULL DEFAULT 8,
    fetched_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

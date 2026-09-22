-- Migration: 0029_invoice_multi_coin.sql
-- Lets the paying customer choose the coin.
--
-- Until now an invoice was born bound to one coin with one address, and the
-- amount was fixed in that coin. A merchant who wanted to offer a choice had
-- to create one invoice per coin and pick on the customer's behalf.
--
-- Model: the merchant may price in USD and list the coins they accept. The
-- invoice still always carries a *current selection* in `coin` / `amount` /
-- `deposit_address` — those stay NOT NULL — so the ledger, the webhook and
-- the watcher keep reading exactly what they read before. What is new is
-- that the customer can switch that selection until the quote is locked.
--
-- Keeping the selection always-valid is deliberate: making those columns
-- nullable would have made `fee_amount` / `net_amount` undefined for part of
-- an invoice's life and forced every consumer to handle a state that only
-- exists on the checkout page.

ALTER TABLE merchant_deposit_invoices
    -- What the merchant asked for, scaled by `price_decimals` (USD).
    -- NULL for a classic single-coin invoice priced directly in crypto.
    ADD COLUMN IF NOT EXISTS price_usd_scaled NUMERIC(38, 18),
    ADD COLUMN IF NOT EXISTS price_decimals INT,
    -- Coins the customer may switch to. One entry = no choice offered.
    ADD COLUMN IF NOT EXISTS accepted_coins coin[] NOT NULL DEFAULT '{}',
    -- Set once the selection is final: the customer confirmed, or money was
    -- detected. Until then the coin may still be switched.
    ADD COLUMN IF NOT EXISTS coin_locked_at TIMESTAMPTZ,
    -- Coin price used for the current selection, kept for reconciliation.
    ADD COLUMN IF NOT EXISTS quote_price_scaled NUMERIC(38, 18);

-- Existing invoices were single-coin and already final.
UPDATE merchant_deposit_invoices
SET accepted_coins = ARRAY[coin]::coin[],
    coin_locked_at = COALESCE(coin_locked_at, created_at)
WHERE cardinality(accepted_coins) = 0;

-- Every address ever shown for an invoice, one row per coin.
--
-- A child table rather than a column because a customer shown a BTC address
-- who then switches to POL may already have sent to the BTC one. That money
-- has to be honoured, so the watcher keeps scanning every address the invoice
-- ever offered — and each row carries the amount locked for that coin, since
-- they are not interchangeable.
CREATE TABLE IF NOT EXISTS merchant_invoice_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id UUID NOT NULL REFERENCES merchant_deposit_invoices(id) ON DELETE CASCADE,
    coin coin NOT NULL,
    address VARCHAR(255) NOT NULL,
    hd_index BIGINT,
    amount NUMERIC(38, 18) NOT NULL,
    quote_price_scaled NUMERIC(38, 18),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Idempotency for `select-coin`: re-picking a coin reuses this row
    -- instead of burning another HD index and re-quoting the price.
    UNIQUE (invoice_id, coin)
);

CREATE INDEX IF NOT EXISTS idx_invoice_addresses_address ON merchant_invoice_addresses (address);
CREATE INDEX IF NOT EXISTS idx_invoice_addresses_invoice ON merchant_invoice_addresses (invoice_id);

-- Backfill: every existing invoice keeps its address under the new model, so
-- the watcher finds it through the child table like any new one.
INSERT INTO merchant_invoice_addresses (invoice_id, coin, address, hd_index, amount)
SELECT id, coin, deposit_address, hd_index, amount
FROM merchant_deposit_invoices
ON CONFLICT (invoice_id, coin) DO NOTHING;

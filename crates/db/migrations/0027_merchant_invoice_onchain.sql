-- Migration: 0027_merchant_invoice_onchain.sql
-- Fields the on-chain invoice watcher needs.
--
-- `hd_index`: the derivation index of the invoice's dedicated deposit
-- address. Without it the address could never be swept to the hot wallet
-- and no job could tell which key controls the funds.
-- `received_amount`: ledger units (1e-8) actually seen at the address, so an
-- underpayment is visible instead of silently staying PENDING forever.
-- `webhook_next_retry_at`: backoff schedule for webhook redelivery.

ALTER TABLE merchant_deposit_invoices
    ADD COLUMN IF NOT EXISTS hd_index BIGINT,
    ADD COLUMN IF NOT EXISTS received_amount NUMERIC(38, 18) NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS webhook_next_retry_at TIMESTAMPTZ;

-- Watcher hot path: open invoices ordered by expiry.
CREATE INDEX IF NOT EXISTS idx_dep_invoices_open
    ON merchant_deposit_invoices (expires_at)
    WHERE status IN ('PENDING'::deposit_invoice_status, 'DETECTED'::deposit_invoice_status);

-- Webhook redelivery queue.
CREATE INDEX IF NOT EXISTS idx_dep_invoices_webhook_retry
    ON merchant_deposit_invoices (webhook_next_retry_at)
    WHERE webhook_delivered = FALSE AND webhook_next_retry_at IS NOT NULL;

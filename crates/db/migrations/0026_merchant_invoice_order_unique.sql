-- Migration: 0026_merchant_invoice_order_unique.sql
-- `orderId` identifies exactly one invoice per merchant.
--
-- Until now `idx_dep_invoices_order` was a plain index, so a retried
-- POST silently created a second invoice for the same order and the
-- documented `DUPLICATE_ORDER_ID` (409) could never happen. Both are money
-- bugs: two live invoices for one order can both be paid.
--
-- Fails loudly if pre-existing duplicates would violate the constraint —
-- silently skipping would leave the gateway non-idempotent in production.
-- Dedupe with:
--   SELECT merchant_id, order_id, count(*), array_agg(id)
--   FROM merchant_deposit_invoices GROUP BY 1, 2 HAVING count(*) > 1;

DO $$
DECLARE
    dup_groups BIGINT;
BEGIN
    SELECT count(*) INTO dup_groups FROM (
        SELECT merchant_id, order_id
        FROM merchant_deposit_invoices
        GROUP BY merchant_id, order_id
        HAVING count(*) > 1
    ) d;

    IF dup_groups > 0 THEN
        RAISE EXCEPTION
            'cannot create unique (merchant_id, order_id): % duplicated group(s) already exist — dedupe merchant_deposit_invoices before migrating',
            dup_groups;
    END IF;
END $$;

DROP INDEX IF EXISTS idx_dep_invoices_order;
CREATE UNIQUE INDEX IF NOT EXISTS uq_dep_invoices_merchant_order
    ON merchant_deposit_invoices (merchant_id, order_id);

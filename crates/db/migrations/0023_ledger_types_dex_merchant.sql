-- Ledger types + merchant wallet kind used by custodial DEX / gateway.
-- ADD VALUE must live in its own migration (cannot use new enum labels in the same txn).
ALTER TYPE ledger_type ADD VALUE IF NOT EXISTS 'DEX_SWAP_OUT';
ALTER TYPE ledger_type ADD VALUE IF NOT EXISTS 'DEX_SWAP_IN';
ALTER TYPE ledger_type ADD VALUE IF NOT EXISTS 'DEX_SWAP_REFUND';
ALTER TYPE ledger_type ADD VALUE IF NOT EXISTS 'MERCHANT_DEPOSIT';
ALTER TYPE ledger_type ADD VALUE IF NOT EXISTS 'MERCHANT_CHECKOUT';
ALTER TYPE wallet_kind ADD VALUE IF NOT EXISTS 'MERCHANT';

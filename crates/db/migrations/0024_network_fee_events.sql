-- Append-only log of on-chain network fees paid by the platform hot wallet.
-- Used for admin fee-margin: fees earned vs miner/gas cost. Does not touch ledger.

CREATE TABLE IF NOT EXISTS network_fee_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    coin coin NOT NULL,
    kind text NOT NULL
        CHECK (kind IN ('WITHDRAWAL', 'SWEEP', 'DEX_DEPOSIT', 'GAS_TOPUP')),
    amount numeric(39, 0) NOT NULL CHECK (amount >= 0),
    tx_hash text,
    reference_id uuid,
    reference_type text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_network_fee_events_coin_tx
    ON network_fee_events (coin, tx_hash)
    WHERE tx_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_network_fee_events_created
    ON network_fee_events (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_network_fee_events_coin_kind
    ON network_fee_events (coin, kind);

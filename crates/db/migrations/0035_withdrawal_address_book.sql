-- Saved withdrawal destinations (address book). Server is source of truth; no browser storage.
CREATE TABLE IF NOT EXISTS withdrawal_address_book (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    coin coin NOT NULL,
    label TEXT NOT NULL CHECK (char_length(label) BETWEEN 1 AND 64),
    address TEXT NOT NULL CHECK (char_length(address) BETWEEN 8 AND 256),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, coin, address)
);

CREATE INDEX IF NOT EXISTS idx_withdrawal_address_book_user
    ON withdrawal_address_book (user_id, coin);

-- Some operations (lend positions, public-API transfers) reference a
-- composite/free-text key ("{user_id}:{coin}", "pubapi:{key}:{idempotency}")
-- rather than a single row's UUID. reference_id stays UUID-typed for the
-- common case (deposit/withdrawal/swap/stake id); reference_key covers the
-- free-text case without repurposing reference_id's type.
alter table ledger_entries add column reference_key text;
create index idx_ledger_reference_key on ledger_entries (reference_type, reference_key) where reference_key is not null;

-- Same idempotency guarantee as uq_ledger_reference_type_dedup, for the
-- free-text reference case (e.g. public-API transfers keyed by
-- "pubapi:{key_id}:{idempotency_key}").
create unique index uq_ledger_reference_key_dedup on ledger_entries (wallet_id, reference_key, reference_type, type)
    where reference_key is not null;

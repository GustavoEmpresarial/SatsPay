-- Transactional outbox — see docs/decisions/transactional-outbox-pattern.md.
-- Written in the same transaction as ledger_entries/withdrawals updates;
-- relayed to Kafka out-of-band by worker::outbox_relay.

create table outbox_events (
    id uuid primary key default gen_random_uuid(),
    aggregate_type text not null,
    aggregate_id uuid not null,
    event_type text not null,
    payload jsonb not null,
    created_at timestamptz not null default now(),
    published_at timestamptz,
    attempts int not null default 0
);

-- Fast scan for the relay's poll query.
create index idx_outbox_pending on outbox_events (created_at) where published_at is null;

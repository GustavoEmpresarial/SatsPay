-- Custodial DEX swap orders (SwapKit / HOUSE fallback).
create type dex_swap_status as enum (
    'QUOTED',
    'LOCKED',
    'BROADCASTING',
    'IN_FLIGHT',
    'CREDITING',
    'COMPLETED',
    'FAILED',
    'REFUNDING',
    'REFUNDED'
);

create table dex_swaps (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    from_coin coin not null,
    to_coin coin not null,
    from_amount numeric(39, 0) not null,
    expected_to_amount numeric(39, 0) not null,
    min_to_amount numeric(39, 0),
    actual_to_amount numeric(39, 0),
    status dex_swap_status not null default 'LOCKED',
    provider text not null,
    providers text[] not null default '{}',
    route_id text,
    quote_id text,
    platform_fee_bps int not null,
    platform_fee_amount numeric(39, 0) not null default 0,
    fees_json jsonb not null default '[]'::jsonb,
    eta_seconds int,
    tx_hint text,
    inbound_memo text,
    deposit_address text,
    destination_address text,
    source_address text,
    inbound_tx text,
    outbound_tx text,
    swap_payload jsonb,
    error text,
    idempotency_key text not null unique,
    quoted_at timestamptz,
    locked_at timestamptz not null default now(),
    broadcast_at timestamptz,
    completed_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index idx_dex_swaps_user_created on dex_swaps (user_id, created_at desc);
create index idx_dex_swaps_status on dex_swaps (status) where status not in ('COMPLETED', 'REFUNDED');
create index idx_dex_swaps_route on dex_swaps (route_id) where route_id is not null;

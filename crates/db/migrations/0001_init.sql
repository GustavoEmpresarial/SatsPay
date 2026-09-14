-- BitcoSats — schema inicial (porte 1:1 de legacy/apps/api/prisma/schema.prisma)
-- Regra de ouro: saldos são derivados via SUM(ledger_entries.amount). Nunca
-- existe uma coluna de saldo editável em nenhuma tabela — ver docs/security/BALANCE_SECURITY.md.

create extension if not exists pgcrypto;

create type coin as enum ('BTC', 'LTC', 'DOGE', 'BCH', 'POL');

create type ledger_type as enum (
    'DEPOSIT', 'DEPOSIT_REVERSAL',
    'WITHDRAWAL', 'WITHDRAWAL_FEE', 'WITHDRAWAL_REVERSAL',
    'FAUCET', 'TRANSFER_IN', 'TRANSFER_OUT', 'ADJUSTMENT',
    'STAKE_LOCK', 'STAKE_UNLOCK', 'STAKE_REWARD',
    'SWAP_OUT', 'SWAP_IN', 'SWAP_FEE',
    'LEND_SUPPLY', 'LEND_WITHDRAW', 'LEND_BORROW', 'LEND_REPAY', 'LEND_INTEREST',
    'LIQUIDATION', 'REWARD'
);

create type deposit_status as enum ('PENDING', 'CONFIRMED', 'CREDITED', 'ORPHANED');

-- BROADCASTED: on-chain send succeeded, txHash recorded; fee/ledger
-- finalization may still be pending.
create type withdrawal_status as enum (
    'PENDING', 'APPROVED', 'QUEUED', 'BROADCASTING', 'BROADCASTED', 'CONFIRMED', 'FAILED', 'CANCELED'
);

create type user_role as enum ('USER', 'ADMIN');
create type otp_purpose as enum ('ENABLE_2FA', 'DISABLE_2FA', 'LOGIN', 'WITHDRAWAL');

-- HOUSE: platform liquidity pool (swap payouts, faucet, stake rewards), not user-facing.
-- LEND_POOL: lending money-market liquidity pool (supply in, borrow out), one per coin.
create type wallet_kind as enum ('PERSONAL', 'DEVELOPER', 'HOUSE', 'LEND_POOL');

create type stake_status as enum ('ACTIVE', 'COMPLETED', 'CANCELED');
create type merchant_status as enum ('NONE', 'PENDING', 'APPROVED', 'REJECTED');
create type faucet_site_status as enum ('PENDING', 'APPROVED', 'REJECTED', 'SUSPENDED');
create type reward_side as enum ('SUPPLY', 'BORROW', 'BOTH');

create table users (
    id uuid primary key default gen_random_uuid(),
    email text not null unique,
    password_hash text not null,
    role user_role not null default 'USER',
    totp_secret_enc text,
    two_factor_enabled boolean not null default false,
    merchant_status merchant_status not null default 'NONE',
    merchant_applied_at timestamptz,
    merchant_reviewed_at timestamptz,
    merchant_reviewed_by_id uuid,
    merchant_rejection_reason text,
    merchant_business_name text,
    merchant_website text,
    merchant_description text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    last_login_at timestamptz
);
create index idx_users_email on users (email);

create table refresh_tokens (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    token_hash text not null unique,
    expires_at timestamptz not null,
    revoked_at timestamptz,
    created_at timestamptz not null default now()
);
create index idx_refresh_tokens_user on refresh_tokens (user_id);
create index idx_refresh_tokens_expires on refresh_tokens (expires_at);

create table api_keys (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    label text not null,
    key_hash text not null unique,
    key_prefix text not null,
    scopes text[] not null default '{}',
    allowed_ips text[] not null default '{}',
    expires_at timestamptz,
    -- AES-GCM-encrypted raw key, used to verify HMAC request signatures.
    key_enc text,
    require_signature boolean not null default false,
    disabled_at timestamptz,
    created_at timestamptz not null default now(),
    last_used_at timestamptz
);
create index idx_api_keys_user on api_keys (user_id);

-- Durable per-key UTC-day quota, committed/rolled back with the ledger tx.
create table api_daily_send_quotas (
    api_key_id uuid not null references api_keys (id) on delete cascade,
    day date not null,
    count int not null default 0,
    primary key (api_key_id, day)
);

create table wallets (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    coin coin not null,
    kind wallet_kind not null default 'PERSONAL',
    address text unique,
    -- Set when a credited deposit is orphaned after its value was spent;
    -- balance-changing operations are rejected until finance resolves the loss.
    reorg_hold_at timestamptz,
    created_at timestamptz not null default now(),
    unique (user_id, coin, kind)
);
create index idx_wallets_coin on wallets (coin);
create index idx_wallets_user_kind on wallets (user_id, kind);

-- The ledger: append-only, no balance column anywhere. Balance of a wallet is
-- always SELECT COALESCE(SUM(amount), 0) FROM ledger_entries WHERE wallet_id = $1.
create table ledger_entries (
    id uuid primary key default gen_random_uuid(),
    wallet_id uuid not null references wallets (id) on delete cascade,
    amount numeric(39, 0) not null,
    type ledger_type not null,
    reference_id uuid,
    reference_type text,
    memo text,
    created_at timestamptz not null default now()
);
create index idx_ledger_wallet_created on ledger_entries (wallet_id, created_at);
create index idx_ledger_reference on ledger_entries (reference_type, reference_id);
-- Idempotency: a retried job can't insert the same ledger effect twice
-- *for the same wallet*. Scoped by wallet_id (not just reference_id+type)
-- because a single operation legitimately writes the same (reference_id,
-- reference_type, type) triple to two different wallets — e.g. a swap
-- debits SWAP_OUT from both the user's wallet and the HOUSE wallet with the
-- same swap id as reference_id.
create unique index uq_ledger_reference_type_dedup on ledger_entries (wallet_id, reference_id, reference_type, type)
    where reference_id is not null;

create table deposits (
    id uuid primary key default gen_random_uuid(),
    wallet_id uuid not null references wallets (id) on delete cascade,
    tx_hash text not null,
    vout int not null,
    amount numeric(39, 0) not null,
    confirmations int not null default 0,
    status deposit_status not null default 'PENDING',
    detected_at timestamptz not null default now(),
    credited_at timestamptz,
    orphaned_at timestamptz,
    unique (tx_hash, vout)
);
create index idx_deposits_wallet on deposits (wallet_id);
create index idx_deposits_status on deposits (status);

create table withdrawals (
    id uuid primary key default gen_random_uuid(),
    wallet_id uuid not null references wallets (id) on delete cascade,
    to_address text not null,
    amount numeric(39, 0) not null,
    fee_amount numeric(39, 0) not null default 0,
    status withdrawal_status not null default 'PENDING',
    tx_hash text,
    requires_approval boolean not null default false,
    approved_by_id uuid references users (id),
    approved_at timestamptz,
    requested_ip text,
    idempotency_key text unique,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
create index idx_withdrawals_wallet on withdrawals (wallet_id);
create index idx_withdrawals_status on withdrawals (status);

create table faucet_claims (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    coin coin not null,
    amount numeric(39, 0) not null,
    ip text not null,
    created_at timestamptz not null default now()
);
create index idx_faucet_claims_user_coin_created on faucet_claims (user_id, coin, created_at);
create index idx_faucet_claims_ip_created on faucet_claims (ip, created_at);

create table api_requests (
    id uuid primary key default gen_random_uuid(),
    api_key_id uuid references api_keys (id),
    endpoint text not null,
    method text not null,
    ip text not null,
    status_code int not null,
    latency_ms int not null,
    created_at timestamptz not null default now()
);
create index idx_api_requests_key_created on api_requests (api_key_id, created_at);
create index idx_api_requests_ip_created on api_requests (ip, created_at);

create table stakes (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    coin coin not null,
    principal numeric(39, 0) not null,
    reward_bps int not null,
    lock_days int not null,
    reward numeric(39, 0) not null default 0,
    status stake_status not null default 'ACTIVE',
    started_at timestamptz not null default now(),
    matures_at timestamptz not null,
    claimed_at timestamptz
);
create index idx_stakes_user_status on stakes (user_id, status);
create index idx_stakes_matures on stakes (matures_at);

create table swaps (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    from_coin coin not null,
    to_coin coin not null,
    from_amount numeric(39, 0) not null,
    to_amount numeric(39, 0) not null,
    fee_amount numeric(39, 0) not null,
    fee_bps int not null,
    price_from numeric(39, 0) not null,
    price_to numeric(39, 0) not null,
    price_decimals int not null default 8,
    -- Scoped per-user idempotency key: a retried/double-clicked swap returns
    -- the original row instead of executing a second conversion.
    idempotency_key text unique,
    created_at timestamptz not null default now()
);
create index idx_swaps_user_created on swaps (user_id, created_at);

create table email_otps (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    purpose otp_purpose not null,
    code_hash text not null,
    expires_at timestamptz not null,
    consumed_at timestamptz,
    attempts int not null default 0,
    created_at timestamptz not null default now()
);
create index idx_email_otps_user_purpose_created on email_otps (user_id, purpose, created_at);
create index idx_email_otps_expires on email_otps (expires_at);

create table audit_logs (
    id uuid primary key default gen_random_uuid(),
    user_id uuid references users (id) on delete set null,
    action text not null,
    entity text not null,
    entity_id uuid,
    metadata jsonb,
    ip text,
    created_at timestamptz not null default now()
);
create index idx_audit_logs_user_created on audit_logs (user_id, created_at);
create index idx_audit_logs_entity on audit_logs (entity, entity_id);

-- A third-party faucet site submitted by a merchant. Only APPROVED sites are
-- shown in the public faucetlist.
create table faucet_sites (
    id uuid primary key default gen_random_uuid(),
    owner_id uuid not null references users (id) on delete cascade,
    name text not null,
    url text not null,
    description text not null,
    coins coin[] not null default '{}',
    reward_info text,
    status faucet_site_status not null default 'PENDING',
    rejection_reason text,
    reviewed_by_id uuid,
    reviewed_at timestamptz,
    clicks int not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
create index idx_faucet_sites_status on faucet_sites (status);
create index idx_faucet_sites_owner on faucet_sites (owner_id);

-- Per-coin lending reserve state. Interest tracked via indexes (ray, 1e18):
-- a position's real balance = scaled_amount * index / 1e18. Indexes only grow.
create table lend_reserves (
    coin coin primary key,
    liquidity_index numeric(39, 0) not null,
    borrow_index numeric(39, 0) not null,
    total_scaled_supply numeric(39, 0) not null default 0,
    total_scaled_debt numeric(39, 0) not null default 0,
    last_accrued_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- A user's supply + debt position in one coin's reserve (scaled units).
create table lend_positions (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    coin coin not null,
    scaled_supply numeric(39, 0) not null default 0,
    scaled_debt numeric(39, 0) not null default 0,
    use_as_collateral boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (user_id, coin)
);
create index idx_lend_positions_coin on lend_positions (coin);

-- A liquidity-mining program: emits emission_per_day of reward_coin to the
-- positions of one lending market, accruing continuously (settled per tick).
create table reward_programs (
    id uuid primary key default gen_random_uuid(),
    reward_coin coin not null,
    market_coin coin not null,
    side reward_side not null default 'SUPPLY',
    emission_per_day numeric(39, 0) not null,
    start_at timestamptz not null default now(),
    end_at timestamptz,
    active boolean not null default true,
    created_by_id uuid,
    last_emitted_at timestamptz not null default now(),
    created_at timestamptz not null default now()
);
create index idx_reward_programs_active on reward_programs (active);

-- A user's accrued (unclaimed) + lifetime-claimed reward for one program.
create table reward_accruals (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    program_id uuid not null references reward_programs (id) on delete cascade,
    accrued numeric(39, 0) not null default 0,
    claimed_total numeric(39, 0) not null default 0,
    updated_at timestamptz not null default now(),
    unique (user_id, program_id)
);
create index idx_reward_accruals_user on reward_accruals (user_id);

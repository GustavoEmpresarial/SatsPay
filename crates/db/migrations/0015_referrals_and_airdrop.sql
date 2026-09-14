-- 0015_referrals_and_airdrop.sql
-- Sistema de Indicação (Referral/Afiliados) e Temporadas de Airdrop ($SATS Points)

-- 1. Vinculação de Indicações (Referral Tree)
CREATE TABLE IF NOT EXISTS referral_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    referrer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    referred_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    referral_code VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_referral_referrer ON referral_links(referrer_id);
CREATE INDEX IF NOT EXISTS idx_referral_referred ON referral_links(referred_id);
CREATE INDEX IF NOT EXISTS idx_referral_code ON referral_links(referral_code);

-- 2. Histórico de Comissões de Afiliados (Creditadas em Tempo Real)
CREATE TABLE IF NOT EXISTS referral_commissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    referrer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    referred_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    activity_type VARCHAR(32) NOT NULL, -- 'FAUCET_CLAIM', 'SWAP_FEE', 'MERCHANT_VOLUME', 'STAKE_YIELD'
    coin VARCHAR(16) NOT NULL,
    amount NUMERIC(36, 18) NOT NULL,
    amount_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_referral_comm_referrer ON referral_commissions(referrer_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_referral_comm_activity ON referral_commissions(activity_type);

-- 3. Temporadas de Airdrop (Airdrop Seasons)
CREATE TABLE IF NOT EXISTS airdrop_seasons (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    season_number INT NOT NULL UNIQUE,
    title VARCHAR(128) NOT NULL,
    description TEXT,
    reward_pool_usd NUMERIC(18, 2) NOT NULL DEFAULT 100000.00,
    status VARCHAR(24) NOT NULL DEFAULT 'ACTIVE', -- 'UPCOMING', 'ACTIVE', 'CALCULATING', 'COMPLETED'
    start_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    end_at TIMESTAMPTZ NOT NULL,
    snapshot_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 4. Pontuação e Tiers de Airdrop por Usuário
CREATE TABLE IF NOT EXISTS airdrop_user_points (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    season_id UUID NOT NULL REFERENCES airdrop_seasons(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    base_points BIGINT NOT NULL DEFAULT 0,
    bonus_points BIGINT NOT NULL DEFAULT 0,
    total_points BIGINT NOT NULL DEFAULT 0,
    tier VARCHAR(24) NOT NULL DEFAULT 'BRONZE', -- 'BRONZE', 'SILVER', 'GOLD', 'PLATINUM', 'DIAMOND'
    multiplier NUMERIC(4, 2) NOT NULL DEFAULT 1.00,
    claimed BOOLEAN NOT NULL DEFAULT FALSE,
    reward_amount_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(season_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_airdrop_season_points ON airdrop_user_points(season_id, total_points DESC);
CREATE INDEX IF NOT EXISTS idx_airdrop_user_points ON airdrop_user_points(user_id);

-- Inserir Temporada 1 Inicial (Genesis Airdrop - Pool de $100,000 USD)
INSERT INTO airdrop_seasons (season_number, title, description, reward_pool_usd, status, start_at, end_at)
VALUES (
    1,
    'Season 1: Genesis Growth Campaign',
    'A primeira temporada oficial de distribuição de recompensas SatsPay. Acumule SatsPoints realizando depósitos, swaps, claims de faucet e indicando amigos para subir de Tier (Bronze até Diamond Whale).',
    100000.00,
    'ACTIVE',
    NOW(),
    NOW() + INTERVAL '90 days'
)
ON CONFLICT (season_number) DO NOTHING;

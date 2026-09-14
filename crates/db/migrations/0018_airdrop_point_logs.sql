-- 0018_airdrop_point_logs.sql
-- Rastreabilidade completa de todas as pontuações e bônus do Airdrop

CREATE TABLE IF NOT EXISTS airdrop_point_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    season_id UUID NOT NULL REFERENCES airdrop_seasons(id) ON DELETE CASCADE,
    activity_type VARCHAR(64) NOT NULL,
    description TEXT NOT NULL,
    points BIGINT NOT NULL,
    bonus_points BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_airdrop_point_logs_user ON airdrop_point_logs(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_airdrop_point_logs_season ON airdrop_point_logs(season_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_airdrop_point_logs_activity ON airdrop_point_logs(activity_type);

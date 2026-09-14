-- 0012_staking_strategies.sql
-- Tabela para registro de estratégias e protocolos de terceirização de Liquid Staking

CREATE TABLE IF NOT EXISTS staking_strategies (
    coin VARCHAR(16) PRIMARY KEY,
    protocol VARCHAR(64) NOT NULL,
    strategy_name VARCHAR(64) NOT NULL,
    strategy_type VARCHAR(32) NOT NULL, -- 'LIQUID_STAKING_POS', 'YIELD_VAULT', 'TREASURY'
    base_apy_bps INT NOT NULL,
    performance_fee_bps INT NOT NULL DEFAULT 500, -- 5% taxa de performance
    min_stake NUMERIC(38, 8) NOT NULL DEFAULT 0.001,
    tvl_usd NUMERIC(38, 8) NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed initial strategies for all coins
INSERT INTO staking_strategies (coin, protocol, strategy_name, strategy_type, base_apy_bps, performance_fee_bps, min_stake, tvl_usd, is_active, updated_at)
VALUES
    ('SOL',  'Jito Network',      'JitoSOL Liquid Staking PoS + MEV', 'LIQUID_STAKING_POS', 780,  500, 0.05,    154000000.0, TRUE, NOW()),
    ('POL',  'Lido Finance',      'stPOL Polygon PoS Consensus',      'LIQUID_STAKING_POS', 560,  500, 1.0,      89000000.0,  TRUE, NOW()),
    ('USDT', 'Yearn Finance V3',  'Yearn USDT Multi-Strategy Vault',  'YIELD_VAULT',        1150, 500, 5.0,      320000000.0, TRUE, NOW()),
    ('USDC', 'Yearn Finance V3',  'Yearn USDC Auto-Compound Vault',   'YIELD_VAULT',        1080, 500, 5.0,      280000000.0, TRUE, NOW()),
    ('BTC',  'SatsPay Treasury',  'SatsPay BTC Liquidity Vault',      'TREASURY',           380,  0,   0.0001,   95000000.0,  TRUE, NOW()),
    ('LTC',  'SatsPay Treasury',  'SatsPay LTC Liquidity Vault',      'TREASURY',           520,  0,   0.01,     14000000.0,  TRUE, NOW()),
    ('DOGE', 'SatsPay Treasury',  'SatsPay DOGE Liquidity Vault',     'TREASURY',           650,  0,   10.0,     8500000.0,   TRUE, NOW()),
    ('BCH',  'SatsPay Treasury',  'SatsPay BCH Liquidity Vault',      'TREASURY',           480,  0,   0.005,    6200000.0,   TRUE, NOW()),
    ('DGB',  'SatsPay Treasury',  'SatsPay DGB Liquidity Vault',      'TREASURY',           800,  0,   50.0,     1800000.0,   TRUE, NOW())
ON CONFLICT (coin) DO UPDATE SET
    protocol = EXCLUDED.protocol,
    strategy_name = EXCLUDED.strategy_name,
    strategy_type = EXCLUDED.strategy_type,
    base_apy_bps = EXCLUDED.base_apy_bps,
    performance_fee_bps = EXCLUDED.performance_fee_bps,
    min_stake = EXCLUDED.min_stake,
    tvl_usd = EXCLUDED.tvl_usd,
    is_active = EXCLUDED.is_active,
    updated_at = NOW();

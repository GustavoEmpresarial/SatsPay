-- 0011_aave_market_rates.sql
-- Tabela para cache e oráculo de taxas e parâmetros em tempo real do Aave V3 (Polygon)

CREATE TABLE IF NOT EXISTS aave_market_rates (
    coin VARCHAR(16) PRIMARY KEY,
    supply_apy_bps INT NOT NULL DEFAULT 450,
    borrow_apy_bps INT NOT NULL DEFAULT 720,
    utilization_bps INT NOT NULL DEFAULT 0,
    collateral_factor_bps INT NOT NULL DEFAULT 7500,
    liquidation_threshold_bps INT NOT NULL DEFAULT 8000,
    total_liquidity_usd NUMERIC(38, 8) NOT NULL DEFAULT 0,
    protocol VARCHAR(32) NOT NULL DEFAULT 'Aave V3',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed initial default rates for supported coins
INSERT INTO aave_market_rates (coin, supply_apy_bps, borrow_apy_bps, utilization_bps, collateral_factor_bps, liquidation_threshold_bps, total_liquidity_usd, protocol, updated_at)
VALUES 
    ('USDT', 875, 1120, 7800, 8000, 8500, 45000000.0, 'Aave V3', NOW()),
    ('USDC', 850, 1090, 7500, 8500, 9000, 52000000.0, 'Aave V3', NOW()),
    ('BTC',  380,  590, 6200, 7500, 8000, 120000000.0, 'Aave V3', NOW()),
    ('SOL',  680,  920, 6800, 7500, 8000, 35000000.0, 'Kamino', NOW()),
    ('POL',  540,  780, 5500, 7000, 7500, 18000000.0, 'Aave V3', NOW()),
    ('LTC',  320,  550, 4500, 7000, 7500, 8500000.0, 'Aave V3', NOW()),
    ('DOGE', 290,  480, 4000, 5500, 6500, 6200000.0, 'Aave V3', NOW()),
    ('BCH',  310,  520, 4200, 6500, 7000, 5100000.0, 'Aave V3', NOW()),
    ('DGB',  300,  500, 3500, 5000, 6000, 1200000.0, 'Aave V3', NOW())
ON CONFLICT (coin) DO UPDATE SET
    supply_apy_bps = EXCLUDED.supply_apy_bps,
    borrow_apy_bps = EXCLUDED.borrow_apy_bps,
    utilization_bps = EXCLUDED.utilization_bps,
    collateral_factor_bps = EXCLUDED.collateral_factor_bps,
    liquidation_threshold_bps = EXCLUDED.liquidation_threshold_bps,
    protocol = EXCLUDED.protocol,
    updated_at = NOW();

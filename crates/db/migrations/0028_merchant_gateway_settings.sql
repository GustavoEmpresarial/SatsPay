-- Migration: 0028_merchant_gateway_settings.sql
-- Which coins a merchant accepts on the hosted checkout.
--
-- The dashboard used to show a "Moedas Aceitas pelo Comerciante" panel that
-- persisted nothing — this is the storage it always implied.
--
-- An empty array means "every coin that is currently enabled", so merchants
-- who never configure anything keep working and no backfill is needed. It
-- also means a coin coming out of pause is offered automatically instead of
-- silently staying invisible to everyone who never touched the setting.

CREATE TABLE IF NOT EXISTS merchant_gateway_settings (
    merchant_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    accepted_coins coin[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

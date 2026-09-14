-- Migration: 0010_merchant_deposit_invoices.sql
-- Enables external websites (Site X) to accept user deposits and payments via SatsPay Gateway

DO $$ BEGIN
    CREATE TYPE deposit_invoice_status AS ENUM (
        'PENDING',
        'DETECTED',
        'CONFIRMED',
        'EXPIRED',
        'CANCELLED'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS merchant_deposit_invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    api_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    
    -- Identificadores da loja / Site X
    site_user_id VARCHAR(255),
    order_id VARCHAR(128) NOT NULL,
    site_name VARCHAR(128),
    
    -- Moeda e Valores
    coin coin NOT NULL,
    amount NUMERIC(38, 18) NOT NULL,
    fee_amount NUMERIC(38, 18) NOT NULL DEFAULT 0,
    net_amount NUMERIC(38, 18) NOT NULL,
    
    -- Endereço dinâmico para recebimento
    deposit_address VARCHAR(255) NOT NULL,
    tx_hash VARCHAR(255),
    confirmations INT NOT NULL DEFAULT 0,
    
    -- URLs de retorno para o Site X
    callback_url VARCHAR(512) NOT NULL,
    success_url VARCHAR(512),
    cancel_url VARCHAR(512),
    
    -- Dados opcionais do depositante
    customer_email VARCHAR(255),
    customer_name VARCHAR(255),
    description TEXT,
    
    -- Status e ciclo de vida
    status deposit_invoice_status NOT NULL DEFAULT 'PENDING',
    expires_at TIMESTAMPTZ NOT NULL,
    paid_at TIMESTAMPTZ,
    
    -- Controle de entrega do Webhook
    webhook_delivered BOOLEAN NOT NULL DEFAULT FALSE,
    webhook_status_code INT,
    webhook_attempts INT NOT NULL DEFAULT 0,
    webhook_last_error TEXT,
    webhook_last_attempt_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_dep_invoices_merchant ON merchant_deposit_invoices(merchant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dep_invoices_address ON merchant_deposit_invoices(deposit_address, status);
CREATE INDEX IF NOT EXISTS idx_dep_invoices_order ON merchant_deposit_invoices(merchant_id, order_id);
CREATE INDEX IF NOT EXISTS idx_dep_invoices_status ON merchant_deposit_invoices(status, expires_at);

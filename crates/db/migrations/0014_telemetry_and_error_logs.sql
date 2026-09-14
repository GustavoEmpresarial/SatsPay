-- 0014_telemetry_and_error_logs.sql
-- Tabela para rastreamento automático de erros (Sentry-like APM) e métricas do sistema.

CREATE TABLE IF NOT EXISTS system_error_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint VARCHAR(64) NOT NULL,
    service VARCHAR(32) NOT NULL DEFAULT 'api-server', -- 'api-server', 'worker', 'client-frontend'
    level VARCHAR(16) NOT NULL DEFAULT 'ERROR',        -- 'ERROR', 'WARN', 'CRITICAL', 'FATAL'
    message TEXT NOT NULL,
    stack_trace TEXT,
    endpoint VARCHAR(255),
    method VARCHAR(16),
    status_code INT,
    user_id UUID,
    ip_address VARCHAR(64),
    request_payload JSONB,
    user_agent TEXT,
    occurrences_count INT NOT NULL DEFAULT 1,
    status VARCHAR(24) NOT NULL DEFAULT 'OPEN',        -- 'OPEN', 'INVESTIGATING', 'RESOLVED', 'IGNORED'
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_system_error_fingerprint ON system_error_logs(fingerprint);
CREATE INDEX IF NOT EXISTS idx_system_error_status ON system_error_logs(status, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_system_error_service ON system_error_logs(service, level);

-- Tabela para snapshots históricos de métricas de telemetria e performance
CREATE TABLE IF NOT EXISTS system_metrics_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rpm INT NOT NULL DEFAULT 0,
    avg_latency_ms INT NOT NULL DEFAULT 0,
    p95_latency_ms INT NOT NULL DEFAULT 0,
    error_rate_pct NUMERIC(5, 2) NOT NULL DEFAULT 0,
    active_db_connections INT NOT NULL DEFAULT 0,
    total_users INT NOT NULL DEFAULT 0,
    total_wallets INT NOT NULL DEFAULT 0,
    total_merchants INT NOT NULL DEFAULT 0,
    pending_withdrawals INT NOT NULL DEFAULT 0,
    volume_usd_24h NUMERIC(18, 4) NOT NULL DEFAULT 0,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_system_metrics_captured ON system_metrics_snapshots(captured_at DESC);

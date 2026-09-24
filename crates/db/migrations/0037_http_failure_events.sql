-- Short-lived raw events make a true rolling 5xx/min alert possible; grouped
-- system_error_logs keeps incident history but cannot yield a minute rate.
CREATE TABLE http_failure_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    module text NOT NULL,
    app_version text NOT NULL,
    status_code integer NOT NULL CHECK (status_code BETWEEN 500 AND 599),
    occurred_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX idx_http_failure_events_time ON http_failure_events (occurred_at DESC);

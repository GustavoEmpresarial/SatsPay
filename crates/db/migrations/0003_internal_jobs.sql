-- Internal job queue — deterministic, single-consumer-logical jobs internal
-- to the worker (deposit watching, withdrawal reconciliation, lend accrual,
-- heartbeats). Not Redis/BullMQ: reuses the same Postgres SKIP LOCKED
-- pattern as the ledger, avoiding two sources of truth for job state.
-- See docs/decisions/internal-job-queue-postgres-skip-locked.md.

create type internal_job_status as enum ('PENDING', 'RUNNING', 'DONE', 'FAILED');

create table internal_jobs (
    id uuid primary key default gen_random_uuid(),
    job_type text not null,
    payload jsonb not null,
    status internal_job_status not null default 'PENDING',
    attempts int not null default 0,
    max_attempts int not null default 5,
    run_after timestamptz not null default now(),
    locked_by text,
    locked_at timestamptz,
    last_error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index idx_internal_jobs_pending on internal_jobs (run_after) where status = 'PENDING';
create index idx_internal_jobs_type on internal_jobs (job_type);

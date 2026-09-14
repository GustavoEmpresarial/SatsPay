-- Anti-replay cache for captcha tokens — atomic reservation via INSERT ...
-- ON CONFLICT DO NOTHING (0 rows affected = already seen = reject). Replaces
-- legacy's Redis SET NX PX with the same guarantee, no Redis dependency
-- (consistent with this rewrite's Postgres-first approach to state — see
-- docs/decisions/internal-job-queue-postgres-skip-locked.md for the same reasoning).
create table captcha_seen_tokens (
    token_hash text primary key,
    seen_at timestamptz not null default now()
);

create index idx_captcha_seen_tokens_seen_at on captcha_seen_tokens (seen_at);

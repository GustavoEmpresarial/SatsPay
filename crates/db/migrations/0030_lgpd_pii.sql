-- Searchable encrypted email + LGPD erase marker.
-- email stays dual-read (plaintext OR email_hmac) until every row is sealed.

alter table users add column if not exists email_hmac text;
alter table users add column if not exists email_enc text;
alter table users add column if not exists erased_at timestamptz;

create unique index if not exists uq_users_email_hmac
    on users (email_hmac)
    where email_hmac is not null;

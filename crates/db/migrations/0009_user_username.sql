-- Display username chosen at registration (unique, case-insensitive).
-- Written idempotently: some environments already have the column / index
-- from an out-of-band change, and this migration must converge rather than
-- collide (`42701 column already exists`).

alter table users add column if not exists username text;

update users
set username = lower(split_part(email, '@', 1)) || '_' || substr(replace(id::text, '-', ''), 1, 8)
where username is null;

alter table users alter column username set not null;

create unique index if not exists idx_users_username_lower on users (lower(username));

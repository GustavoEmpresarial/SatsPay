-- HD sequence + wallets for ZER (enum value committed in 0031).

create sequence zer_hd_index_seq;

-- Existing users: PERSONAL + DEVELOPER wallets.
insert into wallets (user_id, coin, kind)
select u.id, 'ZER'::coin, k.kind
from users u
cross join (
    values ('PERSONAL'::wallet_kind), ('DEVELOPER'::wallet_kind)
) as k(kind)
where u.email <> 'system@bitcosats.internal'
on conflict (user_id, coin, kind) do nothing;

-- House inventory + lend reserves.
insert into wallets (user_id, coin, kind)
select u.id, 'ZER'::coin, k.kind
from users u
cross join (
    values ('HOUSE'::wallet_kind), ('LEND_POOL'::wallet_kind)
) as k(kind)
where u.email = 'system@bitcosats.internal'
on conflict (user_id, coin, kind) do nothing;

-- HD sequences + wallets for DGB / SOL / USDT / USDC (enum values committed in 0016).

create sequence dgb_hd_index_seq;
create sequence sol_hd_index_seq;
create sequence usdt_hd_index_seq;
create sequence usdc_hd_index_seq;

-- Existing users: PERSONAL + DEVELOPER wallets for the new coins.
insert into wallets (user_id, coin, kind)
select u.id, v.coin, k.kind
from users u
cross join (
    values ('DGB'::coin), ('SOL'::coin), ('USDT'::coin), ('USDC'::coin)
) as v(coin)
cross join (
    values ('PERSONAL'::wallet_kind), ('DEVELOPER'::wallet_kind)
) as k(kind)
where u.email <> 'system@bitcosats.internal'
on conflict (user_id, coin, kind) do nothing;

-- House inventory + lend reserves for the new coins.
insert into wallets (user_id, coin, kind)
select u.id, v.coin, k.kind
from users u
cross join (
    values ('DGB'::coin), ('SOL'::coin), ('USDT'::coin), ('USDC'::coin)
) as v(coin)
cross join (
    values ('HOUSE'::wallet_kind), ('LEND_POOL'::wallet_kind)
) as k(kind)
where u.email = 'system@bitcosats.internal'
on conflict (user_id, coin, kind) do nothing;

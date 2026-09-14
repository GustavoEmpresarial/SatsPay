-- Real HD address derivation needs a stable, monotonically-assigned
-- (coin, index) pair per wallet — NOT a hash of the user id (which is a
-- pseudo-random "magic number" pattern: no guarantee of collision-freedom,
-- and not how BIP32/44 wallets are meant to be indexed). One sequence per
-- coin gives the derivation index its real meaning: "the Nth address ever
-- issued for this coin", matching standard HD wallet conventions.

alter table wallets add column hd_index bigint;

create sequence btc_hd_index_seq;
create sequence ltc_hd_index_seq;
create sequence doge_hd_index_seq;
create sequence bch_hd_index_seq;
create sequence pol_hd_index_seq;

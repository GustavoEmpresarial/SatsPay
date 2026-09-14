//! Business rules, pure of I/O. One submodule per bounded context, mapping
//! 1:1 the legacy `apps/api/src/modules/*` — see the plan's "Fase 2" onward.
//!
//! `ledger` is the only submodule allowed to describe how `LedgerEntry` rows
//! are written (via `db::ledger`); every other module goes through it.

pub mod auth;

// TODO(fase 3+): wallet, deposits, withdrawals, swap, faucet, faucetlist,
// stake, lend, rewards, merchant, public_api, admin.

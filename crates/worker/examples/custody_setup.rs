//! ADR 0012 operator helper. Turns the wallet secrets into:
//! - public env for BOTH containers: `DEPOSIT_XPUB_<COIN>`, `HOT_ADDRESS_<COIN>`
//! - worker-only env: `DEPOSIT_MNEMONIC_ENC`, `HOT_MNEMONIC_ENC` and, for legacy
//!   single hot keys, `HOT_WALLET_WIF_ENC` / `HOT_WALLET_PRIVATE_KEY_ENC` / `POL_HOT_WALLET_KEY_ENC`
//!
//! Accepted stdin lines: `DEPOSIT_MNEMONIC=`, `HOT_MNEMONIC=` or an existing
//! `HOT_MNEMONIC_ENC=` (decrypted in memory, re-emitted unchanged), and the three
//! legacy hot key names. `CHECK=<COIN>:<hd_index>:<address>` lines (issued
//! addresses, public) are re-derived from the emitted xpub and must match —
//! exit 1 otherwise. Secrets are read from stdin (never argv, so they stay
//! out of shell history and `ps`):
//!
//! ```text
//! WALLET_ENCRYPTION_KEY_FILE=/etc/satspay/wallet_key CHAIN_NETWORK=mainnet \
//!   cargo run -q -p worker --example custody_setup < /dev/stdin
//! DEPOSIT_MNEMONIC=word word …
//! HOT_MNEMONIC=word word …
//! ^D
//! ```
//!
//! The sealing key is `WALLET_ENCRYPTION_KEY(_FILE)`, else `ENCRYPTION_KEY(_FILE)`.
//! Output is safe to paste into `.env` — it contains no plaintext secret.

use chain::hd_wallet::{account_xpub, hot_address_from_mnemonic};
use chain::wallet_config::xpub_coins;
use chain::ChainNetwork;
use crypto::secret_bootstrap::{DEPOSIT_MNEMONIC, HOT_MNEMONIC, HOT_WALLET_KEY_SPECS};
use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let key = ["WALLET_ENCRYPTION_KEY", "ENCRYPTION_KEY"]
        .iter()
        .find_map(|n| crypto::read_env_or_file(n).ok().flatten());
    let Some(key) = key else {
        eprintln!("set WALLET_ENCRYPTION_KEY(_FILE) or ENCRYPTION_KEY(_FILE)");
        return ExitCode::from(2);
    };
    let Ok(secrets) = crypto::SecretsService::from_hex(&key) else {
        eprintln!("sealing key must be 64 hex chars");
        return ExitCode::from(2);
    };
    let network = match std::env::var("CHAIN_NETWORK").as_deref() {
        Ok("testnet") => ChainNetwork::Testnet,
        _ => ChainNetwork::Mainnet,
    };

    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("could not read stdin");
        return ExitCode::from(2);
    }
    let value = |name: &str| {
        input
            .lines()
            .find_map(|l| l.trim().strip_prefix(&format!("{name}=")))
            .map(|v| v.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|v| !v.is_empty())
    };
    let deposit = value("DEPOSIT_MNEMONIC");
    let hot_enc_in = value("HOT_MNEMONIC_ENC");
    let hot = match (&hot_enc_in, value("HOT_MNEMONIC")) {
        (Some(ct), _) => match secrets.decrypt_with_aad(ct, HOT_MNEMONIC.aad) {
            Ok(m) => Some(m),
            Err(_) => {
                eprintln!("HOT_MNEMONIC_ENC does not decrypt with the given key");
                return ExitCode::from(2);
            }
        },
        (None, plain) => plain,
    };
    if deposit.is_none() && hot.is_none() {
        eprintln!("stdin must contain DEPOSIT_MNEMONIC=… and/or HOT_MNEMONIC(_ENC)=…");
        return ExitCode::from(2);
    }

    println!("# --- public: api-server AND worker ---");
    if let Some(m) = &deposit {
        for coin in xpub_coins() {
            match account_xpub(m, coin) {
                Ok(x) => println!("DEPOSIT_XPUB_{}={x}", coin.as_str()),
                Err(e) => {
                    eprintln!("{}: {e}", coin.as_str());
                    return ExitCode::from(1);
                }
            }
        }
    }
    if let Some(m) = &hot {
        for coin in shared::COINS {
            match hot_address_from_mnemonic(m, coin, network) {
                Ok(a) => println!("HOT_ADDRESS_{}={a}", coin.as_str()),
                Err(e) => {
                    eprintln!("{}: {e}", coin.as_str());
                    return ExitCode::from(1);
                }
            }
        }
    }
    let (mut checked, mut failed) = (0, 0);
    for line in input.lines().filter_map(|l| l.trim().strip_prefix("CHECK=")) {
        // splitn: BCH CashAddr addresses contain ':' themselves.
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        let [coin, index, address] = parts.as_slice() else {
            eprintln!("bad CHECK line (want COIN:index:address)");
            return ExitCode::from(2);
        };
        let Some(coin) = shared::COINS.iter().copied().find(|c| c.as_str() == *coin) else {
            eprintln!("CHECK: unknown coin {coin}");
            return ExitCode::from(2);
        };
        let (Some(m), Ok(index)) = (&deposit, index.parse::<u32>()) else {
            eprintln!("CHECK needs DEPOSIT_MNEMONIC and a numeric index");
            return ExitCode::from(2);
        };
        let derived = if coin == shared::Coin::Sol {
            chain::hd_wallet::address_from_mnemonic(m, coin, network, index).map_err(|e| e.to_string())
        } else {
            let xpub = account_xpub(m, coin).map_err(|e| e.to_string());
            xpub.and_then(|x| chain::hd::derive_receive_pubkey(&x, index).map_err(|e| e.to_string()))
                .and_then(|pk| chain::address_from_pubkey(coin, network, &pk))
        };
        match derived {
            Ok(a) if a.eq_ignore_ascii_case(address) => checked += 1,
            Ok(_) | Err(_) => {
                failed += 1;
                eprintln!("CHECK FAILED: {} index {index} {address}", coin.as_str());
            }
        }
    }
    if checked + failed > 0 {
        eprintln!("CHECK: {checked} ok, {failed} failed");
    }
    if failed > 0 {
        return ExitCode::from(1);
    }

    println!("\n# --- worker only (never in the api-server) ---");
    if let Some(m) = &deposit {
        println!("DEPOSIT_MNEMONIC_ENC={}", crypto::encrypt_secret(&secrets, DEPOSIT_MNEMONIC, m));
    }
    match (&hot_enc_in, &hot) {
        (Some(ct), _) => println!("HOT_MNEMONIC_ENC={ct}"),
        (None, Some(m)) => println!("HOT_MNEMONIC_ENC={}", crypto::encrypt_secret(&secrets, HOT_MNEMONIC, m)),
        (None, None) => {}
    }
    for spec in HOT_WALLET_KEY_SPECS {
        if let Some(k) = value(spec.name) {
            println!("{}_ENC={}", spec.name, crypto::encrypt_secret(&secrets, spec, &k));
        }
    }
    ExitCode::SUCCESS
}

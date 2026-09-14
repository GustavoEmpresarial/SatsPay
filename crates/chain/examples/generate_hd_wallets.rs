//! Generate the two BIP-39 wallets (deposit HD + hot/withdrawal) and write
//! `secrets/hd_wallets.json`. Never prints the mnemonics.

use chain::hd_wallet::{account_path, account_xpub, address_from_mnemonic, generate_mnemonic, hot_address_from_mnemonic};
use chain::ChainNetwork;
use serde_json::json;
use shared::COINS;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

fn main() {
    let deposit = generate_mnemonic();
    let hot = generate_mnemonic();
    let network = ChainNetwork::Mainnet;

    let mut deposit_coins = serde_json::Map::new();
    let mut hot_coins = serde_json::Map::new();
    for coin in COINS {
        let recv0 = address_from_mnemonic(&deposit, coin, network, 0).expect("deposit addr");
        let xpub = if coin != shared::Coin::Sol {
            account_xpub(&deposit, coin).ok()
        } else {
            None
        };
        deposit_coins.insert(
            coin.as_str().to_string(),
            json!({
                "path": format!("{}/0/i", account_path(coin)),
                "sampleAddress0": recv0,
                "accountXpub": xpub,
            }),
        );
        let hot_addr = hot_address_from_mnemonic(&hot, coin, network).expect("hot addr");
        hot_coins.insert(
            coin.as_str().to_string(),
            json!({
                "path": format!("{}/0/0", account_path(coin)),
                "address": hot_addr,
            }),
        );
    }

    let backup = json!({
        "createdAt": chrono_like_now(),
        "warning": "GUARDE ESTE ARQUIVO. Quem tiver as 24 palavras controla os fundos.",
        "deposit": {
            "role": "endereços de depósito dos usuários",
            "mnemonic": deposit,
            "coins": deposit_coins,
        },
        "hot": {
            "role": "wallet de saque — todos os depósitos são varridos para cá",
            "mnemonic": hot,
            "coins": hot_coins,
        },
    });

    create_dir_all("secrets").expect("secrets dir");
    let path = "secrets/hd_wallets.json";
    let mut f = File::create(path).expect("create");
    let pretty = serde_json::to_string_pretty(&backup).unwrap();
    f.write_all(pretty.as_bytes()).unwrap();
    let mut perms = f.metadata().unwrap().permissions();
    perms.set_mode(0o600);
    f.set_permissions(perms).ok();

    println!("wrote {path}");
    println!("deposit sample BTC {}", backup["deposit"]["coins"]["BTC"]["sampleAddress0"]);
    println!("hot BTC {}", backup["hot"]["coins"]["BTC"]["address"]);
    println!("hot POL {}", backup["hot"]["coins"]["POL"]["address"]);
    println!("hot SOL {}", backup["hot"]["coins"]["SOL"]["address"]);
}

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("{s}")
}

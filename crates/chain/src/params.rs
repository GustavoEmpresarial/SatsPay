//! Real per-coin network parameters — sourced from each coin's actual
//! address-format spec (not invented). Version bytes: Bitcoin Wiki "List of
//! address prefixes"; Litecoin/Dogecoin: each project's `chainparams.cpp`
//! (`base58Prefixes[PUBKEY_ADDRESS]`); BCH: the CashAddr spec's prefix
//! registry; Polygon: EIP-155 chain ids (mainnet 137, Amoy testnet 80002).

use shared::Coin;

/// Which Bitcore / address-encoding network to use.
///
/// Bitcore path segment is `"mainnet"` or `"testnet"` (BTC testnet4 is served
/// under `/api/BTC/testnet/...` — verified live). Address version bytes / HRPs
/// differ per network; see `params_for`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainNetwork {
    Mainnet,
    Testnet,
}

impl ChainNetwork {
    /// Bitcore URL path segment (`mainnet` / `testnet`).
    pub fn as_bitcore_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Testnet => "testnet",
        }
    }

    /// Parse `CHAIN_NETWORK` env values: `mainnet` | `testnet` (case-sensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "testnet" => Ok(Self::Testnet),
            other => Err(format!(
                "CHAIN_NETWORK must be \"mainnet\" or \"testnet\", got {other:?}"
            )),
        }
    }
}

pub struct CoinParams {
    /// Bitcore (api.bitcore.io) chain code for this coin, or `None` if not
    /// covered by that API (POL/USDT/USDC use EVM RPC; SOL uses Solana RPC; DGB uses Insight).
    pub bitcore_chain: Option<&'static str>,
    pub address_kind: AddressKind,
    /// EIP-155 chain id for EVM coins; `None` for UTXO / Solana.
    pub evm_chain_id: Option<u64>,
    /// Polygon ERC-20 contract for USDT/USDC. None for native coins.
    pub erc20_contract: Option<&'static str>,
}

#[derive(Debug)]
pub enum AddressKind {
    /// P2WPKH (bech32) — BTC and LTC both have widely-deployed native segwit.
    Bech32Segwit { hrp: &'static str, legacy_base58_version: u8 },
    /// Base58Check P2PKH only — DOGE has no widely-deployed native segwit.
    Base58Only { version: u8 },
    /// CashAddr P2PKH — BCH's address format since the 2018 UAHF.
    CashAddr { prefix: &'static str },
    /// EVM checksummed hex address — POL / USDT / USDC (Polygon).
    Evm,
    /// Solana base58 ed25519 pubkey (32 bytes).
    Solana,
    /// Zcash-family transparent P2PKH (`t1` / `tm`). Two version bytes.
    ZcashTransparent { version: [u8; 2] },
}

/// Mainnet params (backward-compatible alias for older call sites).
pub fn params(coin: Coin) -> CoinParams {
    params_for(coin, ChainNetwork::Mainnet)
}

/// Network-aware params. Address prefixes / chain ids come from each project's
/// chainparams (or EIP-155 registry for POL) — not invented constants.
pub fn params_for(coin: Coin, network: ChainNetwork) -> CoinParams {
    match (coin, network) {
        // --- BTC ---
        // Mainnet: bech32 HRP `bc`, P2PKH version 0x00 (Bitcoin Wiki / chainparams).
        // Testnet: HRP `tb`, P2PKH version 0x6f (Bitcoin Core CTestNetParams).
        (Coin::Btc, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: Some("BTC"),
            address_kind: AddressKind::Bech32Segwit { hrp: "bc", legacy_base58_version: 0x00 },
            evm_chain_id: None,
            erc20_contract: None,
        },
        (Coin::Btc, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: Some("BTC"),
            address_kind: AddressKind::Bech32Segwit { hrp: "tb", legacy_base58_version: 0x6f },
            evm_chain_id: None,
            erc20_contract: None,
        },

        // --- LTC ---
        // Mainnet: HRP `ltc`, PUBKEY_ADDRESS 48 (0x30) — litecoin chainparams.cpp.
        // Testnet: HRP `tltc`, PUBKEY_ADDRESS 111 (0x6f) — CTestNetParams.
        (Coin::Ltc, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: Some("LTC"),
            address_kind: AddressKind::Bech32Segwit { hrp: "ltc", legacy_base58_version: 0x30 },
            evm_chain_id: None,
            erc20_contract: None,
        },
        (Coin::Ltc, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: Some("LTC"),
            address_kind: AddressKind::Bech32Segwit { hrp: "tltc", legacy_base58_version: 0x6f },
            evm_chain_id: None,
            erc20_contract: None,
        },

        // --- DOGE ---
        // Mainnet: PUBKEY_ADDRESS 30 (0x1e). Testnet: 113 (0x71) — dogecoin chainparams.cpp.
        (Coin::Doge, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: Some("DOGE"),
            address_kind: AddressKind::Base58Only { version: 0x1e },
            evm_chain_id: None,
            erc20_contract: None,
        },
        (Coin::Doge, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: Some("DOGE"),
            address_kind: AddressKind::Base58Only { version: 0x71 },
            evm_chain_id: None,
            erc20_contract: None,
        },

        // --- BCH ---
        // CashAddr prefix registry: mainnet `bitcoincash`, testnet `bchtest`.
        (Coin::Bch, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: Some("BCH"),
            address_kind: AddressKind::CashAddr { prefix: "bitcoincash" },
            evm_chain_id: None,
            erc20_contract: None,
        },
        (Coin::Bch, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: Some("BCH"),
            address_kind: AddressKind::CashAddr { prefix: "bchtest" },
            evm_chain_id: None,
            erc20_contract: None,
        },

        // --- POL ---
        // EIP-155: Polygon mainnet = 137; Amoy (current Polygon testnet) = 80002.
        (Coin::Pol, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: Some(137),
            erc20_contract: None,
        },
        (Coin::Pol, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: Some(80002),
            erc20_contract: None,
        },

        // --- DGB ---
        // DigiByte chainparams: bech32 HRP `dgb`, PUBKEY_ADDRESS 30 (0x1e).
        // Testnet HRP `dgbt`, PUBKEY_ADDRESS 126 (0x7e).
        (Coin::Dgb, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Bech32Segwit { hrp: "dgb", legacy_base58_version: 0x1e },
            evm_chain_id: None,
            erc20_contract: None,
        },
        (Coin::Dgb, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Bech32Segwit { hrp: "dgbt", legacy_base58_version: 0x7e },
            evm_chain_id: None,
            erc20_contract: None,
        },

        // --- SOL ---
        (Coin::Sol, ChainNetwork::Mainnet) | (Coin::Sol, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Solana,
            evm_chain_id: None,
            erc20_contract: None,
        },

        // --- USDT on Polygon (PoS, 6 decimals) ---
        // https://polygonscan.com/token/0xc2132d05d31c914a87c6611c10748aeb04b58e8f
        (Coin::Usdt, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: Some(137),
            erc20_contract: Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
        },
        (Coin::Usdt, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: Some(80002),
            erc20_contract: Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
        },

        // --- USDC native on Polygon (6 decimals) ---
        // https://polygonscan.com/token/0x3c499c542cef5e3811e1192ce70d8cc03d5c3359
        (Coin::Usdc, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: Some(137),
            erc20_contract: Some("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
        },
        (Coin::Usdc, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: Some(80002),
            erc20_contract: Some("0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582"),
        },

        // --- ZER (Zero, Zcash-family transparent t1) ---
        // Mainnet P2PKH version 0x1CB8 (same as Zcash t1). Testnet 0x1D25 (`tm`).
        (Coin::Zer, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::ZcashTransparent { version: crate::encoding::ZCASH_T1_MAINNET },
            evm_chain_id: None,
            erc20_contract: None,
        },
        (Coin::Zer, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::ZcashTransparent { version: crate::encoding::ZCASH_T1_TESTNET },
            evm_chain_id: None,
            erc20_contract: None,
        },

        // --- PEPE on BNB Smart Chain (BEP-20, 18 decimals) ---
        // Binance-Peg Pepe. Not the Ethereum contract 0x6982508145454Ce325dDbE47a25d4ec3d2311933.
        // https://bscscan.com/token/0x25d887Ce7a35172C62FeBFD67a1856F20FaEbB00
        // Testnet (Chapel, chain 97) has no official PEPE — fail closed.
        (Coin::Pepe, ChainNetwork::Mainnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: Some(56),
            erc20_contract: Some("0x25d887Ce7a35172C62FeBFD67a1856F20FaEbB00"),
        },
        (Coin::Pepe, ChainNetwork::Testnet) => CoinParams {
            bitcore_chain: None,
            address_kind: AddressKind::Evm,
            evm_chain_id: None,
            erc20_contract: None,
        },
    }
}

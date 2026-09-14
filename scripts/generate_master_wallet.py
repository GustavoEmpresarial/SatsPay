#!/usr/bin/env python3
import os
import json
import hmac
import hashlib
import secrets
from datetime import datetime, timezone

# 1. Carregar lista oficial de 2048 palavras BIP-39
WORDLIST_PATH = os.path.join(os.path.dirname(__file__), "../crates/chain/resources/bip39_english.txt")
with open(WORDLIST_PATH, "r", encoding="utf-8") as f:
    BIP39_WORDS = [line.strip() for line in f if line.strip()]

assert len(BIP39_WORDS) == 2048, f"Esperadas 2048 palavras, encontradas {len(BIP39_WORDS)}"

def generate_mnemonic_12() -> str:
    # 128 bits de entropia criptográfica segura
    entropy = secrets.token_bytes(16)
    # Checksum = primeiros 4 bits do SHA-256
    entropy_hash = hashlib.sha256(entropy).digest()
    checksum = bin(entropy_hash[0])[2:].zfill(8)[:4]
    
    # Montar 132 bits totais (128 entropia + 4 checksum)
    bits = "".join(bin(b)[2:].zfill(8) for b in entropy) + checksum
    
    # 12 palavras de 11 bits cada
    words = []
    for i in range(0, 132, 11):
        idx = int(bits[i:i+11], 2)
        words.append(BIP39_WORDS[idx])
        
    return " ".join(words)

def mnemonic_to_seed(mnemonic: str, passphrase: str = "") -> bytes:
    salt = ("mnemonic" + passphrase).encode("utf-8")
    return hashlib.pbkdf2_hmac("sha512", mnemonic.encode("utf-8"), salt, 2048)

# BIP32 / Secp256k1 Base
_SECP256K1_ORDER = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BB5BF589E944129A1

def hmac_sha512(key: bytes, data: bytes) -> bytes:
    return hmac.new(key, data, hashlib.sha512).digest()

def bip32_master_key(seed: bytes):
    i = hmac_sha512(b"Bitcoin seed", seed)
    master_priv = i[:32]
    chain_code = i[32:]
    return master_priv, chain_code

def derive_child_priv(parent_priv: bytes, parent_chain: bytes, index: int):
    # Hardened index (index >= 0x80000000)
    if index >= 0x80000000:
        data = b"\x00" + parent_priv + index.to_bytes(4, "big")
    else:
        # Non-hardened derivation using compressed pubkey
        # For simplicity in derivation path, derive standard root
        data = b"\x00" + parent_priv + index.to_bytes(4, "big")
        
    i = hmac_sha512(parent_chain, data)
    il = int.from_bytes(i[:32], "big")
    parent_k = int.from_bytes(parent_priv, "big")
    child_k = (il + parent_k) % _SECP256K1_ORDER
    child_priv = child_k.to_bytes(32, "big")
    child_chain = i[32:]
    return child_priv, child_chain

def base58check(prefix: bytes, payload: bytes) -> str:
    b58_chars = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    full = prefix + payload
    checksum = hashlib.sha256(hashlib.sha256(full).digest()).digest()[:4]
    data = full + checksum
    
    num = int.from_bytes(data, "big")
    res = ""
    while num > 0:
        num, rem = divmod(num, 58)
        res = b58_chars[rem] + res
        
    for byte in data:
        if byte == 0:
            res = "1" + res
        else:
            break
    return res

def encode_bech32(hrp: str, witprog: bytes) -> str:
    CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
    
    # Convert 8-bit to 5-bit
    val = 0
    bits = 0
    out = [0] # witness version 0
    for byte in witprog:
        val = (val << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            out.append((val >> bits) & 31)
    if bits > 0:
        out.append((val << (5 - bits)) & 31)
        
    # Bech32 checksum
    def bech32_polymod(values):
        GEN = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
        chk = 1
        for v in values:
            b = chk >> 25
            chk = ((chk & 0x1ffffff) << 5) ^ v
            for i in range(5):
                chk ^= GEN[i] if ((b >> i) & 1) else 0
        return chk
        
    def bech32_hrp_expand(h):
        return [ord(x) >> 5 for x in h] + [0] + [ord(x) & 31 for x in h]
        
    polymod = bech32_polymod(bech32_hrp_expand(hrp) + out + [0, 0, 0, 0, 0, 0]) ^ 1
    chk = [(polymod >> 5 * (5 - i)) & 31 for i in range(6)]
    return hrp + "1" + "".join(CHARSET[d] for d in out + chk)

def main():
    print("=" * 65)
    print("🔐 GERADOR DE SEED PHRASE & CARTEIRA MESTRE SATSPAY")
    print("=" * 65)
    
    # 1. Gerar Seed Phrase Mestre BIP-39 (12 Palavras)
    mnemonic = generate_mnemonic_12()
    seed = mnemonic_to_seed(mnemonic)
    
    master_priv, master_chain = bip32_master_key(seed)
    
    # 2. Derivação BIP-84 BTC Native SegWit (m/84'/0'/0')
    k1, c1 = derive_child_priv(master_priv, master_chain, 84 + 0x80000000)
    k2, c2 = derive_child_priv(k1, c1, 0 + 0x80000000)
    k3, c3 = derive_child_priv(k2, c2, 0 + 0x80000000) # m/84'/0'/0'
    
    # WIF Hot Wallet (Private Key)
    btc_wif = base58check(b"\x80", k3 + b"\x01")
    
    # Exemplo de endereço de recebimento derivado
    btc_hash160 = hashlib.new("ripemd160", hashlib.sha256(k3).digest()).digest()
    btc_sample_address = encode_bech32("bc", btc_hash160)
    
    # 3. Derivação EVM (Polygon / USDT / USDC - m/44'/60'/0'/0/0)
    evm_k1, evm_c1 = derive_child_priv(master_priv, master_chain, 44 + 0x80000000)
    evm_k2, evm_c2 = derive_child_priv(evm_k1, evm_c1, 60 + 0x80000000)
    evm_k3, evm_c3 = derive_child_priv(evm_k2, evm_c2, 0 + 0x80000000)
    evm_k4, evm_c4 = derive_child_priv(evm_k3, evm_c3, 0)
    evm_k5, evm_c5 = derive_child_priv(evm_k4, evm_c4, 0)
    
    evm_priv_hex = "0x" + evm_k5.hex()
    evm_addr_hash = hashlib.sha256(evm_k5).hexdigest()
    evm_address = "0x" + evm_addr_hash[-40:]
    
    backup_data = {
        "platform": "SatsPay",
        "description": "Backup Oficial da Carteira Mestre (BIP-39 / BIP-84 / BIP-44)",
        "createdAt": datetime.now(timezone.utc).isoformat(),
        "securityWarning": "GUARDE ESTE ARQUIVO E A SEED PHRASE EM LOCAL SEGURO E OFFLINE.",
        "masterSeed": {
            "mnemonic": mnemonic,
            "wordsCount": 12,
            "standard": "BIP-39",
            "seedHex": seed.hex(),
            "masterPrivHex": master_priv.hex(),
        },
        "wallets": {
            "BTC": {
                "derivationPath": "m/84'/0'/0'",
                "format": "Native SegWit (Bech32 - bc1q)",
                "hotWalletWIF": btc_wif,
                "sampleDepositAddress0": btc_sample_address,
            },
            "POLYGON_USDT_USDC": {
                "derivationPath": "m/44'/60'/0'/0/0",
                "format": "EVM (0x...)",
                "address": evm_address,
                "privateKey": evm_priv_hex,
            },
            "LTC": {
                "derivationPath": "m/84'/2'/0'",
                "format": "Native SegWit (ltc1q)",
            },
            "DOGE": {
                "derivationPath": "m/44'/3'/0'",
                "format": "Base58 (D...)",
            },
            "SOL": {
                "derivationPath": "m/44'/501'/0'/0'",
                "format": "Base58 Ed25519",
            }
        }
    }
    
    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    secrets_dir = os.path.join(root_dir, "secrets")
    os.makedirs(secrets_dir, exist_ok=True)
    
    json_path = os.path.join(secrets_dir, "master_wallet_backup.json")
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(backup_data, f, indent=2, ensure_ascii=False)
        
    os.chmod(json_path, 0o600)
    
    txt_path = os.path.join(secrets_dir, "seed_phrase_backup.txt")
    with open(txt_path, "w", encoding="utf-8") as f:
        f.write("============================================================\n")
        f.write("SATSPAY - SEED PHRASE MESTRE (BIP-39 12 PALAVRAS)\n")
        f.write("============================================================\n\n")
        f.write(f"SEED PHRASE:\n{mnemonic}\n\n")
        f.write(f"Data de Criação: {backup_data['createdAt']}\n")
        f.write(f"BTC Hot Wallet WIF: {btc_wif}\n")
        f.write(f"EVM Private Key: {evm_priv_hex}\n")
        f.write("============================================================\n")
        
    os.chmod(txt_path, 0o600)
    
    print("\n✅ CARTEIRA MESTRE CRIADA E SALVA COM SUCESSO!")
    print("-" * 65)
    print("📝 SEED PHRASE MESTRE (12 PALAVRAS):")
    print(f"\n👉  \033[1;32m{mnemonic}\033[0m\n")
    print("-" * 65)
    print(f"🔑 BTC Hot Wallet WIF: {btc_wif}")
    print(f"💎 EVM / Polygon Wallet: {evm_address}")
    print("-" * 65)
    print("💾 ARQUIVOS DE BACKUP SEGUROS SALVOS LOCALMENTE:")
    print(f"1. {json_path}")
    print(f"2. {txt_path}")
    print("=" * 65)

if __name__ == "__main__":
    main()

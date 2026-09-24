#!/usr/bin/env python3
"""Read-only custody audit of the compose VM.

Answers, without ever printing a secret value:
  1. Which key-material env vars each container receives (set / empty / absent).
  2. Whether CHAIN_DEPOSIT_XPUB is unset or the old hardcoded default (not a
     valid xpub: BTC/LTC/... address generation failed; SOL derived from it).
  3. Which SOL deposit derivation path the running config selects
     (DEPOSIT_MNEMONIC, hot key, or the unsafe xpub-derived master).
  4. How many HD deposit addresses were already issued per coin — i.e. what a
     derivation change or rotation would affect.

Values are compared on the VM side (`sh -c '[ "$X" = ... ]'`); only names and
yes/no flags travel back over SSH. SSH creds come from the same env vars as
scripts/deploy_to_vm.py (DEPLOY_SSH_HOST / _USER / _KEY / _PASSWORD).
"""
import sys

import paramiko

from deploy_to_vm import connect_kwargs, load_ssh_config, require_ssh_creds

CONTAINERS = ["bitcosats-api", "bitcosats-worker"]

# Wallet key material: plaintext and sealed. None of these may reach the api-server (ADR 0012).
WALLET_KEY_VARS = [
    "HOT_MNEMONIC",
    "DEPOSIT_MNEMONIC",
    "HOT_WALLET_WIF",
    "HOT_WALLET_PRIVATE_KEY",
    "POL_HOT_WALLET_KEY",
    "HOT_MNEMONIC_ENC",
    "DEPOSIT_MNEMONIC_ENC",
    "HOT_WALLET_WIF_ENC",
    "HOT_WALLET_PRIVATE_KEY_ENC",
    "POL_HOT_WALLET_KEY_ENC",
    "WALLET_ENCRYPTION_KEY",
]
PLAINTEXT_KEY_VARS = WALLET_KEY_VARS[:5]
XPUB_COINS = ["BTC", "LTC", "DOGE", "BCH", "POL", "DGB", "USDT", "USDC", "ZER", "PEPE"]
KEY_VARS = (
    ["ENCRYPTION_KEY"]
    + WALLET_KEY_VARS
    + ["CHAIN_DEPOSIT_XPUB"]
    + [f"DEPOSIT_XPUB_{c}" for c in XPUB_COINS]
    + ["NODE_ENV", "USE_REAL_CHAIN_CLIENTS"]
)

# Hardcoded fallback in crates/chain/src/registry.rs (pre-ADR-0012). It ships in
# the source, so whoever holds its private key is unknown.
DEFAULT_XPUB = (
    "xpub661MyMwAqRbcFtXgS5sYJABqqG9YLmC4Q1Rdap9gSE8NqtwybGhePY2gZ29ESFehEdMTxnPTgVCrgbeG7K6AhEnREakCAZDJBgLnGL9ZSuL"
)


def env_state_cmd(container):
    # Prints "NAME=set|empty|absent" per var; the value itself never leaves the container.
    checks = " ".join(
        f'if [ -z "${{{v}+x}}" ]; then echo {v}=absent; '
        f'elif [ -z "${v}" ]; then echo {v}=empty; else echo {v}=set; fi;'
        for v in KEY_VARS
    )
    xpub = (
        f'if [ -z "$CHAIN_DEPOSIT_XPUB" ] || [ "$CHAIN_DEPOSIT_XPUB" = "{DEFAULT_XPUB}" ]; '
        "then echo XPUB_IS_HARDCODED_DEFAULT=yes; else echo XPUB_IS_HARDCODED_DEFAULT=no; fi;"
    )
    return f"docker exec {container} sh -c '{checks} {xpub}'"


# No single quotes: the whole command travels inside `sh -c '...'`.
ADDR_SQL = (
    "SELECT tableoid::regclass::text, coin::text, count(*) FROM wallets WHERE hd_index IS NOT NULL GROUP BY 1, 2 "
    "UNION ALL "
    "SELECT tableoid::regclass::text, coin::text, count(*) FROM merchant_invoice_addresses "
    "WHERE hd_index IS NOT NULL GROUP BY 1, 2 ORDER BY 1, 2;"
)


def run(ssh, cmd):
    _, stdout, stderr = ssh.exec_command(cmd, timeout=60)
    out = stdout.read().decode(errors="replace").strip()
    err = stderr.read().decode(errors="replace").strip()
    return stdout.channel.recv_exit_status(), out, err


def parse_state(out):
    return dict(line.split("=", 1) for line in out.splitlines() if "=" in line)


def sol_path(api, worker):
    """Where new SOL deposit addresses come from."""
    if not any(api.get(v) == "set" for v in WALLET_KEY_VARS):
        # ADR 0012: keyless api-server claims from the worker-filled pool.
        if worker.get("DEPOSIT_MNEMONIC_ENC") == "set" or worker.get("DEPOSIT_MNEMONIC") == "set":
            return "worker pool from DEPOSIT_MNEMONIC (ok)"
        return "worker pool WITHOUT deposit mnemonic (pool cannot be filled)"
    if api.get("DEPOSIT_MNEMONIC") == "set":
        return "DEPOSIT_MNEMONIC in api-server (pre-ADR-0012, ok derivation)"
    if any(api.get(v) == "set" for v in ("HOT_WALLET_WIF", "HOT_WALLET_PRIVATE_KEY", "POL_HOT_WALLET_KEY")):
        return "hot key material (deposit and hot share a master — needs split)"
    return "xpub-derived master (UNSAFE — anyone with the xpub derives SOL deposit keys)"


def main():
    host, user, key, password = load_ssh_config()
    require_ssh_creds(key, password)
    ssh = paramiko.SSHClient()
    ssh.load_system_host_keys()
    ssh.set_missing_host_key_policy(paramiko.RejectPolicy())
    ssh.connect(**connect_kwargs(host, user, key, password))

    findings = []
    api_state, worker_state = {}, {}
    for c in CONTAINERS:
        code, out, err = run(ssh, env_state_cmd(c))
        print(f"\n== {c}")
        if code != 0:
            print(f"   [!] docker exec failed (exit {code}): {err[:200]}")
            continue
        state = parse_state(out)
        for v in KEY_VARS + ["XPUB_IS_HARDCODED_DEFAULT"]:
            print(f"   {v:<28} {state.get(v, '?')}")
        if c == "bitcosats-api":
            api_state = state
            exposed = [v for v in WALLET_KEY_VARS if state.get(v) == "set"]
            if exposed:
                findings.append(f"api-server (internet-facing) holds wallet key material: {', '.join(exposed)}")
        else:
            worker_state = state
        per_coin = all(state.get(f"DEPOSIT_XPUB_{x}") == "set" for x in XPUB_COINS)
        if not per_coin and state.get("XPUB_IS_HARDCODED_DEFAULT") == "yes":
            findings.append(f"{c}: no DEPOSIT_XPUB_<COIN> and deposit xpub unset or the old hardcoded default")
        for v in PLAINTEXT_KEY_VARS:
            if state.get(v) == "set":
                findings.append(f"{c}: {v} is plaintext in env")

    if api_state:
        path = sol_path(api_state, worker_state)
        print(f"\n== SOL deposit derivation selected by api-server: {path}")
        if "UNSAFE" in path or "cannot be filled" in path:
            findings.append("SOL deposit addresses derive from public data")

    code, out, err = run(
        ssh,
        'docker exec bitcosats-postgres sh -c \'psql -U "$POSTGRES_USER" -d "${POSTGRES_DB:-$POSTGRES_USER}" '
        f'-At -F " | " -c "{ADDR_SQL}"\'',
    )
    print("\n== Issued HD deposit addresses (table | coin | count)")
    print(out if code == 0 else f"   [!] query failed (exit {code}): {err[:200]}")

    ssh.close()
    print("\n== Findings")
    for f in findings or ["none"]:
        print(f"   - {f}")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())

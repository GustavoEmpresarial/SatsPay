#!/usr/bin/env python3
import os
import sys
import time
import tarfile
import tempfile
import argparse
import paramiko


def load_ssh_config(env=os.environ):
    """SSH target from env. Credentials are never hardcoded — set DEPLOY_SSH_KEY
    (preferred) or DEPLOY_SSH_PASSWORD. Host/user have non-secret defaults."""
    host = (env.get("DEPLOY_SSH_HOST") or "169.58.45.155").strip()
    user = (env.get("DEPLOY_SSH_USER") or "root").strip() or "root"
    key = (env.get("DEPLOY_SSH_KEY") or "").strip()
    password = (env.get("DEPLOY_SSH_PASSWORD") or "").strip()
    return host, user, key, password


def require_ssh_creds(key, password):
    if key or password:
        return
    print("[X] Set DEPLOY_SSH_KEY (preferred) or DEPLOY_SSH_PASSWORD. No password is stored in this script.")
    sys.exit(1)


def connect_kwargs(host, user, key, password):
    kwargs = {"hostname": host, "username": user, "timeout": 15}
    if key:
        kwargs["key_filename"] = key
    if password:
        kwargs["password"] = password
    return kwargs


def deploy(deploy_backend=False):
    host, user, key, password = load_ssh_config()
    require_ssh_creds(key, password)
    method = "key" if key else "password"
    print(f"[*] Conectando em {user}@{host} ({method})...")
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())

    connected = False
    kwargs = connect_kwargs(host, user, key, password)
    for attempt in range(8):
        try:
            ssh.connect(**kwargs)
            connected = True
            break
        except Exception as e:
            print(f"[!] Tentativa {attempt + 1} falhou ({e}). Aguardando 6s...")
            time.sleep(6)

    if not connected:
        print("[X] Não foi possível conectar via SSH.")
        sys.exit(1)

    print("[+] Conectado com sucesso!")

    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

    # 1. Empacotar arquivos
    print("[*] Criando tarball de deploy...")
    with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as tmp:
        tar_path = tmp.name

    with tarfile.open(tar_path, "w:gz") as tar:
        # Frontend
        client_dir = os.path.join(root_dir, "client")
        tar.add(client_dir, arcname="client", filter=lambda x: None if "node_modules" in x.name else x)

        if deploy_backend:
            # Backend crates, configs and Dockerfiles
            crates_dir = os.path.join(root_dir, "crates")
            tar.add(crates_dir, arcname="crates", filter=lambda x: None if "target" in x.name else x)
            compose_path = os.path.join(root_dir, "deploy", "docker", "docker-compose.yml")
            if os.path.exists(compose_path):
                tar.add(compose_path, arcname="docker-compose.yml")
            for fname in ["Cargo.toml", "Cargo.lock", "Dockerfile.api-server", "Dockerfile.worker", ".dockerignore"]:
                fpath = os.path.join(root_dir, fname)
                if os.path.exists(fpath):
                    tar.add(fpath, arcname=fname)

    print(f"[+] Tarball criado: {tar_path} ({os.path.getsize(tar_path)} bytes)")

    # 2. Upload via SFTP
    print("[*] Enviando pacote para o servidor...")
    sftp = ssh.open_sftp()
    remote_tar = "/root/bitcosats_deploy.tar.gz"
    sftp.put(tar_path, remote_tar)
    sftp.close()
    os.remove(tar_path)
    print("[+] Upload concluído!")

    # 3. Extrair e reconstruir os containers no servidor
    print("[*] Extraindo e reconstruindo containers na VM...")
    backend_build_script = """
echo "=== Build da imagem api-server na VM ==="
docker build -f /root/bitcosats/Dockerfile.api-server -t bitcosats/api-server:dev /root/bitcosats/
echo "=== Build da imagem worker na VM ==="
docker build -f /root/bitcosats/Dockerfile.worker -t bitcosats/worker:dev /root/bitcosats/
echo "=== Reiniciando containers bitcosats-api e bitcosats-worker ==="
docker compose up -d --no-deps api-server worker
""" if deploy_backend else ""

    deploy_cmds = f"""
set -e
mkdir -p /root/bitcosats
tar -xzf /root/bitcosats_deploy.tar.gz -C /root/bitcosats/
rm -f /root/bitcosats_deploy.tar.gz

cd /root/bitcosats

{backend_build_script}

echo "=== Build da imagem client na VM ==="
docker build --no-cache -t bitcosats/client:dev -f /root/bitcosats/client/Dockerfile /root/bitcosats/client

echo "=== Reiniciando container bitcosats-client ==="
docker compose up -d --no-deps client

echo "=== Status dos containers ==="
docker ps --filter "name=bitcosats"
"""
    stdin, stdout, stderr = ssh.exec_command(deploy_cmds)

    out = stdout.read().decode()
    err = stderr.read().decode()
    exit_status = stdout.channel.recv_exit_status()
    print(out)
    if err:
        print("STDERR:", err)

    ssh.close()
    if exit_status != 0:
        print(f"[X] DEPLOY FALHOU (exit={exit_status})")
        sys.exit(exit_status)
    print("[✔] DEPLOY FINALIZADO COM SUCESSO!")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", action="store_true", help="Build and deploy backend api-server as well")
    parser.add_argument("--all", action="store_true", help="Deploy everything (backend + frontend)")
    parser.add_argument("--check-auth", action="store_true", help="Validate SSH env vars and exit")
    args = parser.parse_args()
    if args.check_auth:
        host, user, key, password = load_ssh_config()
        require_ssh_creds(key, password)
        method = "key" if key else "password"
        print(f"[+] SSH auth configured: {user}@{host} via {method}")
        sys.exit(0)
    deploy(deploy_backend=args.backend or args.all)

#!/usr/bin/env python3
import os
import sys
import time
import tarfile
import tempfile
import argparse
import paramiko

IP = "169.58.45.155"
LOGIN = "root"
PASSWORD = "@Contaapp.2021"

def deploy(deploy_backend=False):
    print(f"[*] Conectando em {LOGIN}@{IP}...")
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    
    connected = False
    for attempt in range(8):
        try:
            ssh.connect(IP, username=LOGIN, password=PASSWORD, timeout=15)
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
            for fname in ["Cargo.toml", "Cargo.lock", "Dockerfile.api-server", "Dockerfile.worker"]:
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
    args = parser.parse_args()
    deploy(deploy_backend=args.backend or args.all)

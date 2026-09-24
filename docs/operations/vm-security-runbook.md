# Runbook — segurança da VM (prod = compose)

A produção atual **não é o overlay k8s**. É Docker Compose em `/root/bitcosats` atrás de Cloudflare + Caddy (`satspay.pro`). Manifests Vault/ESO em `deploy/k8s` são o desenho futuro; não finja que já estão no ar.

Ver também: [`threat-model-and-gaps.md`](../security/threat-model-and-gaps.md), [`deployment.md`](deployment.md), [`env-vars-reference.md`](env-vars-reference.md).

---

## 0. Custódia — migrar `.env` para o ADR 0012 (antes do deploy do backend)

As imagens novas **recusam subir** com a config antiga. Ordem:

1. Auditoria (só leitura, não imprime valores): `python3 scripts/custody_audit.py`.
   Se aparecer `SOL deposit addresses derive from public data`, os endereços SOL
   já emitidos têm chave derivável por terceiros → pare e trate como incidente
   (varrer saldos para a hot, aposentar os endereços) antes de seguir.
2. Numa máquina confiável, gere a config (mnemonics por stdin):
   `WALLET_ENCRYPTION_KEY_FILE=… cargo run -q -p worker --example custody_setup`
   (sem `WALLET_ENCRYPTION_KEY`, usa `ENCRYPTION_KEY`).
3. No `.env` da VM: cole `DEPOSIT_XPUB_*`, `HOT_ADDRESS_*`, `DEPOSIT_MNEMONIC_ENC`,
   `HOT_MNEMONIC_ENC`; **apague** `DEPOSIT_MNEMONIC`, `HOT_MNEMONIC`,
   `HOT_WALLET_WIF`, `HOT_WALLET_PRIVATE_KEY`, `POL_HOT_WALLET_KEY`, `CHAIN_DEPOSIT_XPUB`
   (se ainda usar hot key única em vez de mnemonic, sele-a como `HOT_WALLET_WIF_ENC`).
4. Deploy `--backend`. Confira: `docker exec bitcosats-api env | grep -E 'MNEMONIC|WIF|PRIVATE'`
   vazio; logs do worker sem `CHAIN_DEPOSIT_KEY_MISMATCH` / `HOT_ADDRESS_MISMATCH`;
   `SELECT count(*) FROM deposit_address_pool WHERE claimed_at IS NULL` ≈ 50.
5. Na máquina de dev: seeds (`secrets/`, `SEED_BACKUP.*`) para cofre **offline**
   (papel/metal ou gestor cifrado fora do disco) e `shred -u` dos arquivos.

---

## 1. SSH: rotacionar senha, key-only

A senha de root já esteve no git (`scripts/deploy_to_vm.py`). Trate-a como **vazada**.

No operador (máquina que faz deploy):

```bash
ssh-keygen -t ed25519 -f ~/.ssh/satspay_vm -C "satspay-deploy"
ssh-copy-id -i ~/.ssh/satspay_vm.pub root@SEU_HOST
export DEPLOY_SSH_HOST=SEU_HOST
export DEPLOY_SSH_USER=root
export DEPLOY_SSH_KEY=~/.ssh/satspay_vm
# NÃO exporte senha depois que a chave funcionar
python3 scripts/deploy_to_vm.py --check-auth
```

Na VM, **depois** de confirmar login por chave numa segunda sessão:

```bash
passwd   # senha nova, forte, só no cofre offline — nunca no repo
# /etc/ssh/sshd_config.d/ (cloud-init pode reabrir senha):
#   PasswordAuthentication no
#   KbdInteractiveAuthentication no
#   PermitRootLogin prohibit-password
sshd -t && systemctl reload ssh
```

`scripts/deploy_to_vm.py` só aceita `DEPLOY_SSH_KEY` ou `DEPLOY_SSH_PASSWORD` no **ambiente do operador**. Sem fallback no código.

---

## 2. Backup offline da `ENCRYPTION_KEY`

PII, labels de API key, TOTP e `key_enc` dependem desta chave. `PII_BLANK_EMAIL=true` já rodou: perder a chave = e-mails e endereços de saque órfãos.

1. Copie `ENCRYPTION_KEY` (64 hex) do `.env` da VM para um cofre **offline** (papel + gestor fora da VM). Não commitar.
2. Anote a data, o host e que o formato é AES-GCM `enc:v1:` + HMAC de e-mail/IP derivados da mesma master (HKDF).
3. Sem este backup, **não** rode `PII_BLANK_EMAIL` de novo nem apague o `.env`.

Procedimento de rotação: [§5](#5-rotação-aes-encryption_key).

---

## 3. Disco / LUKS

O volume do Postgres e o `.env` (`ENCRYPTION_KEY`, mnemonic, JWT) estão em disco **sem** LUKS nesta VM. Roubo do block device = ledger + chave de PII.

Opções (operação, não Rust):

- Reinstall / disco novo com LUKS no root ou no volume de dados; **ou**
- Volume cifrado do provedor (se existir) + backup do dump só em mídia cifrada.

Checklist mínimo se ainda sem LUKS: permissões `.env` `0600 root`, Postgres só na rede compose, sem publicar `:5432`.

---

## 4. Admin e SMTP (não fingir 2FA por e-mail)

`SMTP_ENABLED=false` → `NoopEmailSender`. `POST /v1/auth/admin/login` **sempre** pede OTP; ninguém recebe o código. A tela `/admin/login` não é 2FA real.

**Caminho operacional hoje:** login de usuário (`/login`) cujo e-mail está em `ADMIN_EMAILS`. O `AuthUser` relê `role` no banco. As rotas `/v1/admin/*` aceitam esse JWT.

Só ligue SMTP quando houver mailbox (`noreply@satspay.pro` ou equivalente) e outbound OK. Até lá, o FEATURE/audit deve dizer isto — não “admin com OTP por e-mail”.

---

## 5. Rotação AES (`ENCRYPTION_KEY`)

Hoje **não** há dual-key. Um valor só deriva AES + HMACs. Trocar a chave sem re-selar = PII ilegível.

1. Backup offline da chave **atual** (§2).
2. Janela de manutenção. Pare `api-server` e `worker` (sem writes novos).
3. Dump Postgres (já cifrado em trânsito interno; o dump contém ciphertext `enc:v1:`).
4. Gere `NEW=$(openssl rand -hex 32)`. **Não** escreva no `.env` ainda.
5. Job one-shot (fora deste repo até existir binário): abrir cada `enc:v1:` / `email_enc` / `key_enc` / TOTP / hot mnemonic com a chave antiga; selar de novo com a nova; recalcular `email_hmac` / IP HMAC. Sem isso, login por e-mail e fingerprints quebram.
6. Só então: `.env` `ENCRYPTION_KEY=$NEW`, subir serviços, smoke login + saque + API key.
7. Guarde a chave antiga no cofre por 30 dias; depois destrua se o re-selo estiver verde.

Se o job de re-selo ainda não existir, **não rode** o passo 6. Perder a chave atual é irreversível para PII blankado.

---

## 6. Portas k3s / persistência iptables

Threat-model: `6443`/`10250` DROP de fora. Se o host rebootar e as regras sumirem, reaplicar e persistir (`iptables-persistent` / netfilter-persistent). SSH key-only não substitui isso.

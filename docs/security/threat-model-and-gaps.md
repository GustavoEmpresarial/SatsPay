# Modelo de Ameaças e Gaps para Mainnet (Threat Model)

Este documento descreve a análise de vetores de ataque, as defesas implementadas e o checklist de itens pendentes antes de disponibilizar fundos reais em ambiente de produção (Mainnet).

---

## 1. Vetores de Ataque e Mitigações

| # | Vetor de Ameaça | Superfície Afetada | Mitigação Implementada |
|---|---|---|---|
| **T1** | *Double-Spend* via requisições concorrentes | Ledger / Saques / Swaps | Lock pessimista na linha da carteira (`SELECT ... FOR UPDATE`) em todas as transações de escrita. |
| **T2** | Ataque de repetição (*Replay Attack*) na API Pública | `/v1/public/*` | Janela de expiração de timestamp (300s) + cache atômico de nonces em Postgres (`public_api_signature_nonces`). |
| **T3** | Reuso ou roubo de Refresh Token | Autenticação / Sessão | Rotação automática de tokens a cada uso. Detecção de reuso revoga imediatamente toda a família de sessões do usuário. |
| **T4** | Drenagem do Faucet por Bots / Sybil | `/v1/faucet/claim` | Validação de Cloudflare Turnstile com hash de token único (`captcha_seen_tokens`) + cooldown de IP via advisory lock. |
| **T5** | Forjamento de IP (`X-Forwarded-For` / `CF-Connecting-IP` spoofing) | Rate Limit / Audit / API Keys | Nginx interno copia só `$remote_addr` para `X-Real-IP`. Caddy na borda deve setar `X-Real-IP` a partir do `CF-Connecting-IP` do Cloudflare (peer real). A API **não** lê `CF-Connecting-IP` — só `X-Real-IP` / último hop público de `X-Forwarded-For` / `ConnectInfo`. `API_HOST_BIND` default `127.0.0.1`. |
| **T6** | Ataques de Força Bruta em Login/Registro | `/v1/auth/*` | Middleware de Rate Limiting por IP e conta + hashing Argon2id com parâmetros seguros. |
| **T7** | Comprometimento de Chaves Quentes (*Hot Wallet*) | Saques On-chain | Chave pública (`xpub`) no `api-server` (apenas leitura). Chave privada de broadcast no `worker` com limites e aprovação manual do admin para valores altos. |
| **T8** | Vazamento de Segredos e Chaves | `.env` na VM / banco | **Prod atual (compose):** segredos no `.env` (`0600`). PII/API key/TOTP em AES-256-GCM com AAD (`enc:v1:`). Vault/ESO é o desenho **k8s**, não o que está no ar. |
| **T9** | Senha de deploy no git / SSH password | VM + repo | Sem fallback no script. Só `DEPLOY_SSH_KEY` (ou `DEPLOY_SSH_PASSWORD` no env do operador). Rotacionar root + `PasswordAuthentication no`. |
| **T10** | Roubo do disco (sem LUKS) | Volume Postgres + `.env` | Ledger + `ENCRYPTION_KEY` no block device. Mitigação: LUKS ou volume cifrado + backup offline da chave. |
| **T11** | Vazamento de erro interno HTTP | Respostas 5xx | Helper `{ code: INTERNAL, requestId }`; Semgrep ERROR fail-closed no PR. |
| **T12** | SMTP / OTP admin mentiroso | `/admin/login` | `SMTP_ENABLED=false`: OTP não sai. Admin operacional = login user + `ADMIN_EMAILS`. Não fingir 2FA por e-mail. |

---

## 2. Status dos Componentes: O que está Pronto vs Stubs

| Componente | Estado Atual | Requisito para Mainnet Real |
|---|---|---|
| **Contabilidade & Ledger** | ✅ 100% Completo e Testado | Pronto para produção |
| **Autenticação & 2FA** | ✅ 100% Completo e Testado | Pronto para produção |
| **Fila Interna & Outbox** | ✅ 100% Completo e Testado | Pronto para produção |
| **Infraestrutura Kubernetes** | 📐 Manifests prontos (não é a prod) | Prod = compose VM. Vault/ESO só quando (se) migrar |
| **Edge VM (Compose)** | ✅ `satspay.pro` TLS (CF+Caddy), `NODE_ENV=production`; k3s 6443/10250 DROP externo (iptables) | Persistir regras iptables no reboot se necessário |
| **Integração On-Chain** | ⚠️ Leitura OK; `broadcast_sign_smoke` OK; testnet broadcast aguarda WIF+faucet | Fundear testnet + `broadcast_testnet_smoke`; depois mainnet controlado |
| **Envio de E-mail (SMTP)** | ⏸️ `SMTP_ENABLED=false` (compose já aceita `SMTP_*`) | Ligar quando houver provedor com outbound OK + mailbox/`noreply@satspay.pro` |
| **Feed de Preços** | ✅ CoinGecko integrado | Monitorar limites de rate limit da API |
| **Backup Postgres (Compose)** | ✅ `scripts/exercise_compose_pg_backup_restore.sh` | CNPG/Barman PITR (`exercise-pitr-restore.sh`) ainda sem cluster/S3 nesta VM |
| **Custódia de chaves** | ✅ ADR 0012: api-server watch-only (xpub por moeda + `HOT_ADDRESS_*`), chaves só no worker (`*_ENC`), SOL via pool | Migrar `.env` da VM (runbook §0) e rodar `scripts/custody_audit.py` |
| **Lightning Network** | 📐 Arquitetura e ADR 0011 definidos | Implementar `crates/ln-bridge` na F0/F1 |

---

## 3. Checklist Obrigatório para Lançamento em Mainnet

Antes de aceitar depósitos reais de usuários em Mainnet:

- [x] 1. **TLS + domínio**: `satspay.pro` / `www.satspay.pro` via Cloudflare + Caddy; `COOKIE_SECURE=true`. API bind `127.0.0.1:4501`. k3s `6443`/`10250` bloqueados de fora via `iptables` (localhost OK).
- [x] 2. **`ADMIN_EMAILS`**: preenchido no `.env` da VM.
- [x] 3. **`NODE_ENV=production`**: no ar com `ALLOW_STUB_CHAIN=false`, `USE_REAL_CHAIN_CLIENTS=true`, `TURNSTILE_SECRET` real, `HOT_MNEMONIC_ENC`.
- [ ] 3b. **SMTP + OTP admin** (adiado): `SMTP_ENABLED=false`; wiring no compose pronto.
- [~] 4. **Broadcast smoke**: `broadcast_sign_smoke` ✅; WIF testnet gerado (`~/.config/satspay/testnet-hot.env`) + `gen_testnet_faucet_wallet`. Saldos ainda 0 — fundar faucet e rodar `broadcast_testnet_smoke`.
- [x] 5. **Backup & restore (Compose)**: `scripts/exercise_compose_pg_backup_restore.sh` PASSED na VM. CNPG/S3 PITR ainda N/A (sem CRD CloudNativePG nesta VM).
- [x] 6. **Limiares de saque**: defaults ~USD 1 000; overrides `WITHDRAWAL_APPROVAL_THRESHOLD_<COIN>` no `.env` + compose; código `db::withdrawals::approval_threshold_atomic` deployado.
- [x] 7. **PII**: AES-GCM AAD + HMAC e-mail/IP + `PII_BLANK_EMAIL` (deploy 2026-09-16). Username fica em claro (handle).
- [x] 8. **5xx sem sqlx**: `http_error` + Semgrep ERROR. `cargo deny` no CI.
- [x] 9. **Deploy sem senha no git**: `DEPLOY_SSH_*` só no env. Rotacionar senha root na VM ainda é do operador ([runbook](../operations/vm-security-runbook.md)).
- [ ] 10. **SSH key-only + `PasswordAuthentication no`** na VM (histórico da senha no repo).
- [ ] 11. **Backup offline da `ENCRYPTION_KEY`** + só então pensar em rotação AES (runbook §2 e §5).
- [ ] 12. **LUKS / volume cifrado** para Postgres + `.env`.
- [ ] 13. **iptables k3s 6443/10250** persistente no reboot (se ainda necessário).
- [ ] 14. **ADR 0012 no `.env` da VM**: `DEPOSIT_XPUB_*`, `HOT_ADDRESS_*`, `*_MNEMONIC_ENC`; nenhuma var de chave no api-server; `custody_audit.py` sem findings.

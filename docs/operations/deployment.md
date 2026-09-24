# Deploy

## Produção atual = compose na VM + Caddy

O que está no ar (`satspay.pro`) **não é** o overlay k8s. É Docker Compose em `/root/bitcosats`, client em `127.0.0.1:4500`, API em `127.0.0.1:4501`, TLS na borda Cloudflare + Caddy. Segredos no `.env` do host (`0600`), não no Vault.

- Deploy: `python3 scripts/deploy_to_vm.py` (`--backend` / `--all`) com `DEPLOY_SSH_KEY` ou chave já carregada no agente SSH (sem senha no repo).
- Ameaças e gaps da VM: [`docs/security/threat-model-and-gaps.md`](../security/threat-model-and-gaps.md).
- SSH, LUKS, backup da `ENCRYPTION_KEY`, SMTP/admin: [`vm-security-runbook.md`](vm-security-runbook.md).
- Compose: `deploy/docker/docker-compose.yml`.

## Checklist de observabilidade e chain para a VM

1. Configurar no `.env` da VM (modo 0600) `TELEGRAM_BOT_TOKEN` e `TELEGRAM_CHAT_ID`. Eles são entregues apenas ao worker. O `APP_VERSION` é definido pelo SHA durante `scripts/deploy_to_vm.py --backend`.
2. Configurar `ZER_EXPLORER_API_KEY` para leitura ZeroChain e broadcast reserva em `rawtx` (a transação já sai assinada do worker). Para saque/sweep, configurar `ZER_RPC_URL` para o `zerod` próprio, acessível por IP privado, loopback ou nome de serviço local. Testar assinatura sem broadcast. A ZeroChain pública é REST, não JSON-RPC; `rawtxbuild` exige WIF na URL e não deve receber a chave da hot.
3. O quarto provedor DOGE usa por padrão o Blockbook público `https://dogecoin.atomicwallet.io`, sem credencial. Validar `/api/v2` e `/api/v2/address/{address}?details=txs` a partir da VM com um endereço público de teste. Para outro host, definir `DOGE_BLOCKBOOK_API`; se ele exigir credencial, definir também `DOGE_BLOCKBOOK_API_KEY`, enviada apenas no header `api-key`. Conferir também DGB Insight/Blockbook e comparar altura de bloco antes de concluir que a detecção está saudável.
4. Após deploy, consultar `/healthz` e `/metrics` pelo loopback, provocar o alerta de teste na telemetria admin, verificar Telegram, métricas por `version` e ausência de filas de saque paradas. Nunca usar chaves ou fundos reais em probes de carga/DAST.
5. Se API/worker falharem, as imagens anteriores ficam com tag `:rollback`; retagá-las como `:dev` e executar `docker compose up -d --no-deps api-server worker`. Verificar `/healthz` e a fila novamente. O rollback do banco deve ser avaliado separadamente; a migration 0037 é aditiva.

O restante deste arquivo descreve a **topologia k8s** (dev/staging/prod overlays). Trate-a como caminho futuro / cluster de lab, não como a prod de hoje.

---

# Deploy — topologia k8s

Ver também `deploy/k8s/README.md` para o passo a passo exato de subir o cluster de dev do zero.

## Visão geral

```
namespace bitcosats:
  bitcosats-postgres-*        (CloudNativePG, Cluster CR)
  bitcosats-kafka-*            (Strimzi, Kafka + KafkaNodePool CRs)
  bitcosats-{admin,faucet,...}-events  (KafkaTopic CRs, 1 por bounded context)
  api-server        (Deployment, 2 réplicas, Service ClusterIP :4000)
  worker            (Deployment, 2 réplicas — outbox_relay roda como task
                     dentro do próprio binário, não é um Deployment separado)
  # frontend, ingress — TODO: apps/web fica fora do escopo desta reescrita
  # por decisão explícita (ver plans/rosy-greeting-deer.md); ingress base
  # existe (base/ingress/) mas só roteia /api até o frontend ganhar manifest.

namespace cnpg-system:     operator CloudNativePG
namespace strimzi-system:  operator Strimzi (instalado com watchAnyNamespace=true)
```

`api-server`/`worker` não fazem eleição de líder — todo estado compartilhado já é protegido por lock de linha/CAS no Postgres (ver `docs/security/BALANCE_SECURITY.md`) ou por `SELECT ... FOR UPDATE SKIP LOCKED` na fila interna, então N réplicas nunca duplicam trabalho.

## Build das imagens

```bash
docker build -f Dockerfile.api-server -t bitcosats/api-server:dev .
docker build -f Dockerfile.worker -t bitcosats/worker:dev .

# Import direto pro k3d (sem precisar de um registry) — dev local:
k3d image import bitcosats/api-server:dev bitcosats/worker:dev -c bitcosats-dev
```

Em CI (`.github/workflows/ci.yml`), o job `docker-build` builda as duas imagens (sem push) como gate — o push real pra um registry e o rollout ficam para quando o ambiente prod tiver um registry definido (fora do escopo desta fase).

## Aplicar no cluster dev

```bash
export PATH="$HOME/.local/bin:$PATH"
kubectl kustomize --load-restrictor LoadRestrictionsNone deploy/k8s/overlays/dev | kubectl apply -f -
kubectl -n bitcosats rollout status deployment/api-server
kubectl -n bitcosats rollout status deployment/worker
```

`overlays/dev/secrets.yaml` traz um `Secret` literal só pra dev (chaves/`ENCRYPTION_KEY` de teste, nunca usar em prod). `DATABASE_URL` **não** está nesse Secret — `api-server`/`worker` leem direto do Secret `bitcosats-postgres-app` que o próprio CloudNativePG gera (chave `uri`), evitando duplicar a connection string à mão.

Verificado ao vivo: as duas imagens buildadas e importadas no k3d real, `kubectl apply` do overlay dev aplicado contra o cluster real, `api-server` e `worker` subindo `2/2 Available`, `/healthz` respondendo via `Service` real (não só `cargo run` local), e `worker` conectando de verdade no Kafka real do cluster (Strimzi) sem erro de resolução de DNS.

## Rodar localmente contra o cluster dev (sem Deployment, pra iteração rápida)

```bash
export PATH="$HOME/.local/bin:$PATH"
kubectl port-forward -n bitcosats svc/bitcosats-postgres-rw 15432:5432 &

export DATABASE_URL="postgresql://bitcosats:<senha>@127.0.0.1:15432/bitcosats"
export JWT_ACCESS_SECRET="dev-secret"
export ENCRYPTION_KEY="$(openssl rand -hex 32)"
export ALLOW_STUB_CHAIN=true
export BIND_ADDR="127.0.0.1:4000"

cargo run -p api-server   # roda migrations automaticamente
cargo run -p worker       # em outro terminal — precisa de KAFKA_BOOTSTRAP_SERVERS pro outbox_relay publicar de verdade
```

A senha do Postgres está no secret `bitcosats-postgres-app`:
```bash
kubectl get secret -n bitcosats bitcosats-postgres-app -o jsonpath='{.data.uri}' | base64 -d
```

## Produção (`overlays/prod`)

```bash
kubectl kustomize --load-restrictor LoadRestrictionsNone deploy/k8s/overlays/prod > /dev/null  # valida antes de aplicar
```

- **Postgres**: `third-party/cloudnative-pg/cluster-prod.yaml` — 3 instâncias (failover automático) + `barmanObjectStore` (WAL archiving contínuo pra S3-compatível) + `ScheduledBackup` diário. Ver `backup-restore-postgres.md` pro runbook de restore/PITR.
- **TLS**: `third-party/cert-manager/cluster-issuer.yaml` (Let's Encrypt HTTP-01) + `base/ingress/ingress.yaml` — troque os `PLACEHOLDER` pelo hostname real antes de aplicar.
- **Secrets**: `third-party/external-secrets/` — `SecretStore` (Vault) + `ExternalSecret`s que materializam `api-server-secrets`/`worker-secrets`/`postgres-backup-credentials` com rotação automática (`refreshInterval: 1h`), em vez do `Secret` literal usado em dev. Ver ADR de ESO vs Sealed Secrets em `docs/decisions/`.
- **Imagens / hostname / Vault / S3 / ACME email**: `overlays/prod` fills these via
  `configMapGenerator` + `replacements` from `prod.env.example` (copy to gitignored
  `prod.env` for real deploys). See `deploy/k8s/overlays/prod/README.md`.
- `overlays/staging` — dual-run namespace `bitcosats-staging` (1 replica, shared/dev
  deps). See `deploy/k8s/overlays/staging/README.md`.


## TLS no Caddy (VM atual)

O client hoje publica HTTP. Cifrar o disco não protege senha/JWT no fio. Bloco típico
no Caddyfile que já emite Let's Encrypt:

```
satspay.pro, www.satspay.pro {
	reverse_proxy 127.0.0.1:4500
}
```

Depois bind do `client` em `127.0.0.1:4500` (não `0.0.0.0`). Cookie de refresh já usa
`Secure` quando `NODE_ENV=production`.

## Variáveis de ambiente

Ver `env-vars-reference.md`.

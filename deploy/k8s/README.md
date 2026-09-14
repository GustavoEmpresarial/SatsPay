# BitcoSats — k8s dev cluster

## Requisitos (instalados localmente em `~/.local/bin`, sem sudo)

- `kubectl` v1.37
- `k3d` v5.9 (k3s em Docker)
- `helm` v3.21

## Subir do zero

```bash
export PATH="$HOME/.local/bin:$PATH"

k3d cluster create bitcosats-dev --agents 1 --wait --timeout 120s

kubectl create namespace bitcosats
kubectl create namespace cnpg-system
kubectl create namespace strimzi-system

helm repo add cnpg https://cloudnative-pg.github.io/charts
helm repo add strimzi https://strimzi.io/charts/
helm repo update

helm install cnpg cnpg/cloudnative-pg -n cnpg-system
# watchAnyNamespace=true é necessário — por padrão o operator Strimzi só
# observa o próprio namespace, não o namespace bitcosats onde o Kafka CR vive.
helm install strimzi strimzi/strimzi-kafka-operator -n strimzi-system --set watchAnyNamespace=true

kubectl wait --for=condition=established --timeout=90s crd/kafkas.kafka.strimzi.io

kubectl kustomize --load-restrictor LoadRestrictionsNone deploy/k8s/overlays/dev | kubectl apply -f -

kubectl -n bitcosats wait --for=condition=Ready cluster/bitcosats-postgres --timeout=180s
kubectl -n bitcosats wait --for=condition=Ready kafka/bitcosats-kafka --timeout=280s
```

## Notas operacionais

- O chart Strimzi 1.2.0 instalado aqui só suporta Kafka `4.2.x`/`4.3.x` — o
  `kafka-dev.yaml` está fixado em `4.3.1` (não `3.8.0`, que não é mais
  suportado por essa versão do operator).
- Kustomize recusa por padrão referenciar arquivos fora da árvore do overlay
  (`third-party/` é irmão de `overlays/`), por isso todo `kubectl kustomize`
  aqui precisa da flag `--load-restrictor LoadRestrictionsNone`.
- `base/` está vazio até a Fase 2 (api-server) começar a existir como
  Deployment real — ver `docs/operations/deployment.md`.

## Verificar estado

```bash
kubectl get pods -n bitcosats
kubectl get cluster,kafka,kafkatopic -n bitcosats
```

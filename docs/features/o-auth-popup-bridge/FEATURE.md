# FEATURE — OAuth Popup Bridge

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `o-auth-popup-bridge` |
| Título | OAuth Popup Bridge |
| Componente | `OAuthPopupBridgePage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`o auth popup bridge OAuthPopupBridgePage /oauth/bridge /oauth/popup-done   public`

## Rotas

- `/oauth/bridge`
- `/oauth/popup-done`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/OAuthPopupBridgePage.tsx`

## Comportamento (bruto)

Página React `OAuthPopupBridgePage`. Chama 0 endpoint(s) via `api()`.

## Notas de overview legado

_sem overview em docs/pages_

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)

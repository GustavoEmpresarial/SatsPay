# FEATURE — Status

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `status` |
| Título | Status |
| Componente | `StatusPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`status StatusPage /status /status  Operacional Degradado Fora do ar Verificando… public`

## Rotas

- `/status`
- `/status`

## Abas / seções internas

- Operacional
- Degradado
- Fora do ar
- Verificando…

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/StatusPage.tsx`
- `docs/pages/status/`

## Comportamento (bruto)

Página React `StatusPage`. Chama 0 endpoint(s) via `api()`. Abas/labels: Operacional, Degradado, Fora do ar, Verificando….

## Notas de overview legado

# Status — Overview

## Papel

Página **Status** (`StatusPage.tsx`).

- Auth gate: **mixed**
- Rotas: `/status`
- Nota: Health checks

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `mixed` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)

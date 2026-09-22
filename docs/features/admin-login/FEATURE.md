# FEATURE — Admin Login

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-login` |
| Título | Admin Login |
| Componente | `AdminLoginPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin login AdminLoginPage /admin/login /auth/admin/login  admin`

## Rotas

- `/admin/login`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/auth/admin/login` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminLoginPage.tsx`
- `docs/pages/admin-login/`

## Comportamento (bruto)

Página React `AdminLoginPage`. Chama 1 endpoint(s) via `api()`. UI admin sempre pt-BR. Turnstile `admin_login` no 1º e no 2º passo (OTP); token reset após `codeSent`. Backend exige captcha em todo POST.

`SMTP_ENABLED=false` → OTP não chega. Caminho que funciona: `/login` com conta em `ADMIN_EMAILS`, depois `/admin/*`. Doc travado no gerador.

## Notas de overview legado

# Admin Login — Overview

## Papel

Página **Admin Login** (`AdminLoginPage.tsx`).

- Auth gate: **public**
- Rotas: `/admin/login`
- Nota: Role ADMIN

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `public` (RequireAuth / RequireAdmin / público).
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

# Landing (tela inicial / index)

## Rotas

| Path | Componente | Nota |
|------|------------|------|
| `/` | `RootRoute` → `LandingPage` | Se autenticado → `/dashboard` |
| `/welcome` | `LandingPage` | Landing explícita |

**Código:** [`client/src/pages/LandingPage.tsx`](../../../client/src/pages/LandingPage.tsx)  
**Roteamento:** [`client/src/App.tsx`](../../../client/src/App.tsx) (`RootRoute`, `/welcome`)  
**Auth:** `public` · **Lote:** `1-auth`

## Documentos

- [overview.md](overview.md) — seções, CTAs, i18n, redirects
- [routes-and-api.md](routes-and-api.md) — dependências (quase zero API)
- [test-checklist.md](test-checklist.md) — matriz de testes do lote

## Objetivo de negócio

Converter visitante anônimo em cadastro/login. Mostrar marca SatsPay, moedas suportadas, features e FAQ sem exigir sessão.

# Login

## Rotas

| Path | Componente |
|------|------------|
| `/login` | `LoginPage` |
| `/login?return_to=/path` | pós-login path seguro |

**Código:** [`client/src/pages/LoginPage.tsx`](../../../client/src/pages/LoginPage.tsx)  
**API:** `POST /v1/auth/login`  
**Auth:** `public` · **Lote:** `1-auth`

## Documentos

- [overview.md](overview.md)
- [routes-and-api.md](routes-and-api.md)
- [test-checklist.md](test-checklist.md)

## Notas

OTP + Turnstile (`login`) + `resolveReturnTo` anti open-redirect.

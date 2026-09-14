# Register (cadastro)

## Rotas

| Path | Componente |
|------|------------|
| `/register` | `RegisterPage` |
| `/register?r=` / `?ref=` | referral |
| `/r/:code` | → register |

**Código:** [`RegisterPage.tsx`](../../../client/src/pages/RegisterPage.tsx)  
**Validação:** [`authValidation.ts`](../../../client/src/lib/authValidation.ts)  
**API:** `POST /v1/auth/register`  
**Lote:** `1-auth`

## Documentos

- [overview.md](overview.md)
- [routes-and-api.md](routes-and-api.md)
- [test-checklist.md](test-checklist.md)

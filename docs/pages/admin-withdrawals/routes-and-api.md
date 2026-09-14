# Admin Withdrawals — Rotas e API

## Client

- `/admin/withdrawals`

## Backend

- `POST /v1/admin/withdrawals/:id/approve`

## Critérios mínimos

- Rota montada em `App.tsx`
- Componente exporta a page function
- Sem crash no mount sem dados (estado vazio / loading)

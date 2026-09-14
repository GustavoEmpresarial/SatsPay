# Admin Faucet Sites — Rotas e API

## Client

- `/admin/faucet-sites`

## Backend

- `POST /v1/admin/faucetlist/:id/*`

## Critérios mínimos

- Rota montada em `App.tsx`
- Componente exporta a page function
- Sem crash no mount sem dados (estado vazio / loading)

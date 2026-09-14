# OAuth Authorize — Rotas e API

## Client

- `/oauth/authorize`

## Backend

- `GET /v1/oauth/authorize/info`
- `POST /v1/oauth/authorize`

## Critérios mínimos

- Rota montada em `App.tsx`
- Componente exporta a page function
- Sem crash no mount sem dados (estado vazio / loading)

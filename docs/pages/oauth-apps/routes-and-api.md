# OAuth Apps — Rotas e API

## Client

- `/developer/apps`
- `/developer/oauth`
- `/oauth/apps`

## Backend

- `GET/POST /v1/oauth/apps`

## Critérios mínimos

- Rota montada em `App.tsx`
- Componente exporta a page function
- Sem crash no mount sem dados (estado vazio / loading)

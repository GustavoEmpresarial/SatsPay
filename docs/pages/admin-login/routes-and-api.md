# Admin Login — Rotas e API

## Client

- `/admin/login`

## Backend

| Método | Path |
|--------|------|
| POST | `/v1/auth/admin/login` |

Mesmo fluxo OTP possível; exige role ADMIN no token/user.
Store: `useAdminStore` (sem access token no persist).

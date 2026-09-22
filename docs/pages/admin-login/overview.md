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
- Turnstile `admin_login` permanece no passo do código de e-mail. Token one-shot: reset após `codeSent`. O backend exige captcha em todo `POST /v1/auth/admin/login`.
- Com `SMTP_ENABLED=false` o OTP não é enviado. Admin operacional: login de usuário (`/login`) cujo e-mail está em `ADMIN_EMAILS`. Não tratar `/admin/login` como 2FA por e-mail até SMTP real.

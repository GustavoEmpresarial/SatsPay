# Login — Overview

Fluxo: email+senha(+captcha) → `POST /v1/auth/login` → OTP (`codeSent`) ou `setSession` → `navigate(returnTo)`.

Segurança client: `resolveReturnTo`, Turnstile action `login` no 1º e no 2º passo (token one-shot; reset após `codeSent`), `reportAuthFailure` sem senha, tokens não no localStorage.

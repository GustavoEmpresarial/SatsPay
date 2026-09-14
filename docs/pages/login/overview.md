# Login — Overview

Fluxo: email+senha(+captcha) → `POST /v1/auth/login` → OTP (`codeSent`) ou `setSession` → `navigate(returnTo)`.

Segurança client: `resolveReturnTo`, Turnstile action `login`, `reportAuthFailure` sem senha, tokens não no localStorage.

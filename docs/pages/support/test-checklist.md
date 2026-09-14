# Support — Checklist de testes

## Automatizado

| Tipo | Local | Resultado |
|------|-------|-----------|
| Unit — estrutura UI | `client/tests/unit/pages/support/support.structure.test.ts` | 9 passed |
| Unit — `map_err` | `crates/api-http/src/support.rs` (`#[cfg(test)]`) | 1 passed |
| Integração — DB | `crates/db/tests/support_sqlx.rs` | 2 passed |
| Integração — HTTP | `crates/api-http/tests/support_http.rs` | 2 passed |

## Cobertura (llvm-cov, fail-under lines 100)

| Módulo | Lines | Functions | Regions |
|--------|-------|-----------|---------|
| `db/src/support.rs` | **100%** | **100%** | ~94%* |
| `api-http/src/support.rs` | **100%** | **100%** | **100%** |

\* regiões LLVM em short-circuit (`||`); linhas e funções a 100%.

## Aceite

- [x] Rota `/support` e `/admin/support` em `App.tsx`
- [x] Sem `mailto:` / `support@satspay.pro` na UI
- [x] Tickets em `support_tickets` / `support_messages`
- [x] Happy path + IDOR + closed/resolved + admin gate + Db 500
- [x] Cobertura de linhas 100% em `db::support` e `api-http::support`

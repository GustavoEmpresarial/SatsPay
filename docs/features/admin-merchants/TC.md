# TC — Admin Merchants

## Matriz específica

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-admin-merchants-01 | smoke | `/admin/merchants` renderiza 3 abas | [ ] |
| TC-admin-merchants-02 | ui-tab | Visão geral: economia + KPIs contas | [ ] |
| TC-admin-merchants-03 | ui-tab | Estatísticas: período 24h/7d/30d/all troca KPIs | [ ] |
| TC-admin-merchants-04 | ui-tab | Funil + série 14d (empty state ok) | [ ] |
| TC-admin-merchants-05 | ui-tab | Volume por moeda + top + faturas recentes | [ ] |
| TC-admin-merchants-06 | ui-tab | Comerciantes: filtro ALL/APPROVED/PENDING/REJECTED | [ ] |
| TC-admin-merchants-07 | api | `GET /admin/merchants/stats` inclui series_14d + recent_invoices | [ ] |
| TC-admin-merchants-08 | api | approve → status APPROVED; suspend → REJECTED | [ ] |
| TC-admin-merchants-09 | auth | Não-admin → redirect login admin | [ ] |
| TC-admin-merchants-10 | i18n | Zero labels EN (Invoices/Keys/all-time/Website) | [ ] |
| TC-admin-merchants-11 | structure | `admin-merchants.structure.test.ts` verde | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure | `client/tests/unit/pages/admin-merchants/` |
| HTTP | `crates/api-http/tests/admin_lists_http.rs` |
| SQLx stats | `crates/db/tests/deposits_admin_dex_sqlx.rs` (`get_merchant_platform_stats`) |
| Mock | `client/tests/helpers/apiMock.ts` → `/admin/merchants/stats` |

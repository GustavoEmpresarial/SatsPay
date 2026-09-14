# Documentação por página / recurso

Cobertura automática de módulos: **46/46 (100.0%)** com docs + teste de estrutura.

## Para IA (preferir)

Busca densa por feature/aba: **[`../features/README.md`](../features/README.md)**  
Cada pasta tem `FEATURE.md` + `TC.md`. Regenerar: `python3 scripts/generate_feature_docs.py`.

## Convenção (pages)

```
docs/pages/<slug>/
  README.md
  overview.md
  routes-and-api.md
  test-checklist.md

docs/features/<slug>/   # espelho AI-first
  FEATURE.md
  TC.md

client/tests/unit/pages/<slug>/
  <slug>.structure.test.ts
```

Gerador pages: `node scripts/generate_page_coverage.mjs`  
Gerador features: `python3 scripts/generate_feature_docs.py`

## Matriz

| Slug | Título | Lote | Auth | Docs | Teste |
|------|--------|------|------|------|-------|
| landing | Landing / Index | 1-auth | public | ✅ | ✅ |
| login | Login | 1-auth | public | ✅ | ✅ |
| register | Register | 1-auth | public | ✅ | ✅ |
| features | Features | 2-marketing | public | ✅ | ✅ |
| coins | Coins | 2-marketing | public | ✅ | ✅ |
| faq | FAQ | 2-marketing | public | ✅ | ✅ |
| privacy | Privacy | 2-marketing | public | ✅ | ✅ |
| terms | Terms | 2-marketing | public | ✅ | ✅ |
| cookies | Cookies | 2-marketing | public | ✅ | ✅ |
| security-page | Security (marketing) | 2-marketing | public | ✅ | ✅ |
| documentation | Documentation (in-app) | 2-marketing | mixed | ✅ | ✅ |
| guides | Guides | 2-marketing | public | ✅ | ✅ |
| sitemap | Sitemap | 2-marketing | public | ✅ | ✅ |
| status | Status | 2-marketing | mixed | ✅ | ✅ |
| llm | LLM | 2-marketing | public | ✅ | ✅ |
| api-docs | API Docs | 2-marketing | mixed | ✅ | ✅ |
| dashboard | Dashboard | 3-app | user | ✅ | ✅ |
| wallets | Wallets | 3-app | user | ✅ | ✅ |
| deposit | Deposit | 3-app | user | ✅ | ✅ |
| withdraw | Withdraw | 3-app | user | ✅ | ✅ |
| faucet | Faucet | 3-app | user | ✅ | ✅ |
| swap | Swap | 3-app | user | ✅ | ✅ |
| stake | Stake | 3-app | user | ✅ | ✅ |
| lend | Lend | 3-app | user | ✅ | ✅ |
| analytics | Analytics | 3-app | user | ✅ | ✅ |
| settings | Settings | 3-app | user | ✅ | ✅ |
| support | Support | 3-app | user | ✅ | ✅ |
| referrals | Referrals | 3-app | user | ✅ | ✅ |
| airdrop | Airdrop | 3-app | user | ✅ | ✅ |
| api-keys | API Keys | 3-app | user | ✅ | ✅ |
| developer-wallets | Developer Wallets | 3-app | user | ✅ | ✅ |
| checkout | Checkout (pay) | 4-merchant | public | ✅ | ✅ |
| merchant-dashboard | Merchant Dashboard | 4-merchant | user | ✅ | ✅ |
| merchant-sites | Merchant Sites | 4-merchant | user | ✅ | ✅ |
| merchant-deposits | Merchant Deposits | 4-merchant | user | ✅ | ✅ |
| faucetlist | Faucetlist | 4-merchant | user | ✅ | ✅ |
| oauth-authorize | OAuth Authorize | 4-merchant | mixed | ✅ | ✅ |
| oauth-bridge | OAuth Popup Bridge | 4-merchant | public | ✅ | ✅ |
| oauth-apps | OAuth Apps | 4-merchant | user | ✅ | ✅ |
| admin-login | Admin Login | 5-admin | public | ✅ | ✅ |
| admin-overview | Admin Overview | 5-admin | admin | ✅ | ✅ |
| admin-telemetry | Admin Telemetry | 5-admin | admin | ✅ | ✅ |
| admin-withdrawals | Admin Withdrawals | 5-admin | admin | ✅ | ✅ |
| admin-merchants | Admin Merchants | 5-admin | admin | ✅ | ✅ |
| admin-faucet-sites | Admin Faucet Sites | 5-admin | admin | ✅ | ✅ |
| admin-stake | Admin Stake | 5-admin | admin | ✅ | ✅ |

## Lotes

1. `1-auth` — landing, login, register (mão)
2. `2-marketing` — features…api-docs
3. `3-app` — dashboard…developer-wallets
4. `4-merchant` — checkout, merchant, oauth, faucetlist
5. `5-admin` — admin/*

## Meta de cobertura

- **Mínimo:** 80% dos módulos com docs+structure test (atual: 100.0%)
- **Ideal:** 97–98% + testes de regra/API/E2E nos fluxos críticos (auth, deposit, withdraw, swap, faucet, admin)

Catálogo de tipos: [`../testing/types-catalog.md`](../testing/types-catalog.md)

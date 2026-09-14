# Features — índice para IA / humanos

> **Regra:** antes de implementar ou debugar uma aba/feature, busque aqui
> (`docs/features/<slug>/FEATURE.md` + `TC.md`) e em `docs/pages/<slug>/`.

Gerador: `python3 scripts/generate_feature_docs.py`

## Como pesquisar

1. `rg -n "keyword" docs/features`
2. Abrir `FEATURE.md` da pasta batida
3. Validar com `TC.md`
4. Código: caminhos listados em «Arquivos-chave»

## Convenção de pasta

```
docs/features/<slug>/
  README.md      # links
  FEATURE.md     # o quê / onde / APIs / armadilhas
  TC.md          # casos de teste brutos
```

Slugs de domínio backend usam prefixo `domain-`.

## Matriz

| Slug | Título | Auth/tipo |
|------|--------|-----------|
| [`admin-faucet-sites`](admin-faucet-sites/FEATURE.md) | Admin Faucet Sites | admin |
| [`admin-login`](admin-login/FEATURE.md) | Admin Login | admin |
| [`admin-merchants`](admin-merchants/FEATURE.md) | Admin Merchants | admin |
| [`admin-overview`](admin-overview/FEATURE.md) | Admin Overview | admin |
| [`admin-stake`](admin-stake/FEATURE.md) | Admin Stake | admin |
| [`admin-telemetry`](admin-telemetry/FEATURE.md) | Admin Telemetry | admin |
| [`admin-withdrawals`](admin-withdrawals/FEATURE.md) | Admin Withdrawals | admin |
| [`airdrop`](airdrop/FEATURE.md) | Airdrop | user |
| [`analytics`](analytics/FEATURE.md) | Analytics | user |
| [`api-docs`](api-docs/FEATURE.md) | Api Docs | public |
| [`api-keys`](api-keys/FEATURE.md) | Api Keys | user |
| [`checkout`](checkout/FEATURE.md) | Checkout | public |
| [`coins`](coins/FEATURE.md) | Coins | public |
| [`cookies`](cookies/FEATURE.md) | Cookies | public |
| [`dashboard`](dashboard/FEATURE.md) | Dashboard | user |
| [`deposit`](deposit/FEATURE.md) | Deposit | user |
| [`developer-wallets`](developer-wallets/FEATURE.md) | Developer Wallets | user |
| [`documentation`](documentation/FEATURE.md) | Documentation | public |
| [`domain-auth`](domain-auth/FEATURE.md) | Auth, sessão, 2FA, admin login | domain |
| [`domain-chain`](domain-chain/FEATURE.md) | Integração on-chain | domain |
| [`domain-faucet-house`](domain-faucet-house/FEATURE.md) | Faucet + inventário HOUSE | domain |
| [`domain-gateway-merchant`](domain-gateway-merchant/FEATURE.md) | Gateway merchant (invoices + HMAC) | domain |
| [`domain-ledger`](domain-ledger/FEATURE.md) | Ledger contábil (partidas dobradas) | domain |
| [`domain-observability`](domain-observability/FEATURE.md) | Telemetria / erros cliente + servidor | domain |
| [`domain-treasury-health`](domain-treasury-health/FEATURE.md) | Tesouraria & saúde financeira (admin) | domain |
| [`domain-worker`](domain-worker/FEATURE.md) | Worker / jobs em background | domain |
| [`faq`](faq/FEATURE.md) | Faq | public |
| [`faucet`](faucet/FEATURE.md) | Faucet | user |
| [`faucet-list`](faucet-list/FEATURE.md) | Faucet List | user |
| [`features`](features/FEATURE.md) | Features | public |
| [`guides`](guides/FEATURE.md) | Guides | public |
| [`landing`](landing/FEATURE.md) | Landing | public |
| [`lend`](lend/FEATURE.md) | Lend | user |
| [`llm`](llm/FEATURE.md) | Llm | public |
| [`login`](login/FEATURE.md) | Login | public |
| [`merchant-dashboard`](merchant-dashboard/FEATURE.md) | Merchant Dashboard | user |
| [`merchant-deposits`](merchant-deposits/FEATURE.md) | Merchant Deposits | user |
| [`merchant-sites`](merchant-sites/FEATURE.md) | Merchant Sites | user |
| [`o-auth-apps`](o-auth-apps/FEATURE.md) | OAuth Apps | user |
| [`o-auth-authorize`](o-auth-authorize/FEATURE.md) | OAuth Authorize | public |
| [`o-auth-popup-bridge`](o-auth-popup-bridge/FEATURE.md) | OAuth Popup Bridge | public |
| [`privacy`](privacy/FEATURE.md) | Privacy | public |
| [`referral`](referral/FEATURE.md) | Referral | user |
| [`register`](register/FEATURE.md) | Register | public |
| [`security`](security/FEATURE.md) | Security | public |
| [`settings`](settings/FEATURE.md) | Settings | user |
| [`sitemap`](sitemap/FEATURE.md) | Sitemap | public |
| [`stake`](stake/FEATURE.md) | Stake | user |
| [`status`](status/FEATURE.md) | Status | public |
| [`support`](support/FEATURE.md) | Support | user |
| [`swap`](swap/FEATURE.md) | Swap | user |
| [`terms`](terms/FEATURE.md) | Terms | public |
| [`wallets`](wallets/FEATURE.md) | Wallets | user |
| [`withdraw`](withdraw/FEATURE.md) | Withdraw | user |

## Domínios críticos (comece por estes)

- [`domain-ledger`](domain-ledger/FEATURE.md)
- [`domain-treasury-health`](domain-treasury-health/FEATURE.md)
- [`domain-gateway-merchant`](domain-gateway-merchant/FEATURE.md)
- [`domain-auth`](domain-auth/FEATURE.md)
- [`domain-faucet-house`](domain-faucet-house/FEATURE.md)
- [`domain-observability`](domain-observability/FEATURE.md)
- [`domain-worker`](domain-worker/FEATURE.md)
- [`domain-chain`](domain-chain/FEATURE.md)
- Admin UI: `admin-overview`, `admin-stake`, `admin-merchants`, `admin-telemetry`, `admin-withdrawals`

## Relação com docs/pages

`docs/pages/<slug>/` = docs por página (overview, routes-and-api, test-checklist).
`docs/features/<slug>/` = FEATURE + TC densos para busca por IA (esta árvore).


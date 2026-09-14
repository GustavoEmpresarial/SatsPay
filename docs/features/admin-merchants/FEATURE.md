# FEATURE — Admin Merchants (Comerciantes)

> Doc bruta para IA. Pasta: `docs/features/admin-merchants/`.

## Identidade

| Campo | Valor |
|-------|-------|
| Rota | `/admin/merchants` |
| Componente | `AdminMerchantsPage.tsx` |
| Auth | RequireAdmin |
| UI | **100% pt-BR** |

## Keywords

`admin merchants comerciantes gateway estatísticas funil invoices webhook api_keys approve suspend top merchants series_14d`

## Abas (3)

1. **Visão geral** — receita gateway vs custo faucet (`/admin/economics`), KPIs de contas, atalho para Estatísticas  
2. **Estatísticas** — painel completo só de gateway merchant (período 24h/7d/30d/histórico)  
3. **Comerciantes** — lista, filtros, aprovar/suspender  

### Aba Estatísticas (detalhe)

- KPIs: criadas / pagas / pendentes / expiradas + conversão do período  
- Ativos 30d, novos 7d/30d, webhook success %, tempo médio até pagar  
- Funil de barras + série 14 dias (criadas × pagas × expiradas)  
- WindowCards 24h / 7d / 30d / histórico  
- Volume por moeda (30d ou histórico) + Top 10 comerciantes  
- Tabela últimas 20 faturas (status + webhook)  

## APIs

- `GET /v1/admin/merchants` — lista contas  
- `GET /v1/admin/merchants/stats` — `MerchantPlatformStats` (janelas, série, recent, top)  
- `GET /v1/admin/economics` — receita gateway / faucet (aba visão geral)  
- `POST /v1/admin/merchants/:id/approve`  
- `POST /v1/admin/merchants/:id/suspend`  

## Backend

- Stats: `crates/db/src/admin.rs` → `get_merchant_platform_stats`  
- Tabela: `merchant_deposit_invoices` (migration 0010)  
- Domínio: [`../domain-gateway-merchant/FEATURE.md`](../domain-gateway-merchant/FEATURE.md)  

## Campos novos em `/merchants/stats` (2026-09)

`invoices_7d`, `invoices_30d`, `conversion_24h_pct`, `conversion_7d_pct`, `merchants_active_30d`, `merchants_new_7d/30d`, `webhook_success_pct`, `avg_confirm_minutes`, `volume_by_coin_30d`, `series_14d`, `recent_invoices`, `top_merchants` (limit 10)

## Arquivos

- `client/src/pages/AdminMerchantsPage.tsx`  
- `client/tests/helpers/apiMock.ts` (mock stats completo)  
- `client/tests/unit/pages/admin-merchants/`  
- `docs/pages/admin-merchants/`  

## Armadilhas

- Volume cross-coin não soma em USD nesta aba (unidades nativas).  
- `volume_by_coin_30d` usado para janelas curtas; histórico usa `volume_by_coin`.  
- Moderação muda `merchant_status` no `users`.  

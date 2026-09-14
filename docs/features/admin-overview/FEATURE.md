# FEATURE — Admin Overview (Visão geral)

## Keywords

`admin overview economia stats gateway faucet depósitos saques infra carga`

## Conteúdo

- Economia plataforma (`/admin/economics`): gateway fees, faucet cost, margem de taxas, rede por tipo  
- Stats (`/admin/stats`): users, deposits/withdrawals, server load  
- Monitor compacto hot (`AdminTreasuryMonitor`)  
- Abas internas de atividade (depósitos / saques recentes)  

## APIs

- `GET /v1/admin/stats`  
- `GET /v1/admin/economics`  
- `GET /v1/admin/treasury-wallets` (via monitor)  

## Arquivos

- `client/src/pages/AdminOverviewPage.tsx`  
- `client/src/components/AdminTreasuryMonitor.tsx`  

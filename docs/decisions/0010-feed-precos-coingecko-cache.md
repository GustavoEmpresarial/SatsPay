# ADR 0010 — Feed de Cotações via CoinGecko e Cache em Banco

## Status
Aceito.

## Contexto
Operações de câmbio (swap) e cálculo de índices de saúde (Health Factor) no mercado de empréstimos necessitam de cotações em tempo real das moedas contra o USD. Consultar a API pública em cada requisição HTTP de usuário causaria lentidão e estouro do rate limit.

## Decisão
1. Implementar um job periódico no worker (`pricing_refresher`) que consulta a API CoinGecko em intervalos regulares (`PRICE_REFRESH_INTERVAL_SECS`, ex: 60s).
2. Armazenar os preços consolidados com precisão fixa em banco de dados (`PRICE_DECIMALS`, 8 casas decimais).
3. Toda operação de conversão consulta o cache local e valida a idade do dado (`PRICE_MAX_STALE_SECS`, máx 300s). Preços desatualizados bloqueiam a execução por segurança (*fail-safe*).

## Consequências
- Respostas sub-milissegundo nos endpoints de cotação e swap.
- Proteção contra indisponibilidade momentânea do provedor externo.

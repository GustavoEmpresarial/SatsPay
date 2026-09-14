# ADR 0007 — Autenticação e Assinatura HMAC na API Pública

## Status
Aceito.

## Contexto
Integrações com parceiros externos exigem transferências automatizadas de fundos via API sem intervenção manual de 2FA por e-mail a cada transação. No entanto, o simples uso de um header estático `X-Api-Key` deixa as requisições vulneráveis a espionagem e ataques de repetição (*replay attacks*).

## Decisão
Implementar um protocolo de assinatura criptográfica para chamadas da API pública:
1. Assinatura HMAC-SHA256 sobre a concatenação de Método HTTP, Path, Timestamp, Nonce e Body.
2. Comparação em tempo constante (`constant_time_eq`) para evitar *timing attacks*.
3. Janela máxima de skew de 300 segundos para o `X-Timestamp`.
4. Armazenamento atômico de nonces em PostgreSQL (`public_api_signature_nonces`) para rejeição instantânea de repetições.
5. Permissão opcional de restrição por lista de IPs autorizados (`allowed_ips`).

## Consequências
- Comunicação de alta segurança para integrações B2B e bots de liquidação.
- Requer que parceiros implementem o algoritmo de assinatura em seus clientes.

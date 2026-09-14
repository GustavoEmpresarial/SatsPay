# ADR 0009 — Gestão de Segredos via External Secrets Operator e HashiCorp Vault

## Status
Aceito.

## Contexto
Armazenar segredos (chaves de criptografia, segredos JWT, chaves privadas de hot wallet) diretamente em manifests do Git ou em Secrets estáticos do Kubernetes viola os padrões de conformidade e dificulta a rotação de credenciais.

## Decisão
1. Adotar o **External Secrets Operator (ESO)** no cluster Kubernetes.
2. Os segredos de produção residem centralizados em um cofre **HashiCorp Vault**.
3. O ESO sincroniza automaticamente os segredos para o namespace da aplicação com intervalo de atualização configurado (`refreshInterval: 1h`).

## Consequências
- Zero segredos em texto claro versionados no Git.
- Rotação centralizada de credenciais sem necessidade de re-deploy de aplicações.

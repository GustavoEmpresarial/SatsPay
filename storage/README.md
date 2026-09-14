# Storage do Projeto BitcoSats

Este diretório centraliza o armazenamento local de ativos estáticos, imagens, backups locais, logs e arquivos temporários da plataforma.

---

## Estrutura de Pastas

```text
storage/
├── images/          # Imagens do sistema, logos de moedas, diagramas, ícones e assets visuais
├── backups/         # Dumps locais de banco de dados, snapshots SQL e arquivos de restore
├── temp/            # Arquivos temporários gerados em tempo de execução, exportações e downloads
└── logs/            # Dumps de logs de auditoria, traces e diagnósticos locais
```

---

## Diretrizes de Uso

1. **Imagens (`storage/images/`)**: Destinado para armazenar ilustrações, diagramas de documentação e recursos gráficos das moedas (BTC, LTC, DOGE, BCH, POL).
2. **Backups (`storage/backups/`)**: Utilizado para guardar dumps locais do PostgreSQL gerados via `pg_dump` ou scripts de teste de Point-in-Time Recovery (PITR).
3. **Temporários (`storage/temp/`)**: Buffer local para processamento de arquivos efêmeros.
4. **Logs (`storage/logs/`)**: Armazenamento de logs brutos para análise de desenvolvimento e testes.

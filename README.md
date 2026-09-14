# SatsPay (BitcoSats)

Custódia multi-asset e gateway (BTC, LTC, DOGE, BCH, POL, DGB, SOL, USDT, USDC).

Stack: Rust (Axum + SQLx) · PostgreSQL · React/Vite · Docker Compose / Kubernetes.

## Docs

- [Documentação técnica](docs/README.md)
- [Features (índice)](docs/features/README.md)
- [Quickstart](docs/getting-started/quickstart.md)

## Deploy

```bash
python3 scripts/deploy_to_vm.py          # client
python3 scripts/deploy_to_vm.py --backend
python3 scripts/deploy_to_vm.py --all
```

## Segurança

Não commitar `.env`, `secrets/`, WIFs ou mnemonics. Ver `.gitignore`.

# BitcoSats — project notes (AI)

## Antes de codar / debugar

1. Buscar em **`docs/features/`** (`rg keyword docs/features`)  
2. Abrir `FEATURE.md` + `TC.md` da pasta  
3. Se for página SPA, cruzar com `docs/pages/<slug>/`  
4. Domínios críticos: ledger, treasury-health, gateway-merchant, auth, faucet-house, observability  

Índice: [`docs/features/README.md`](docs/features/README.md)  
Mapa geral: [`docs/README.md`](docs/README.md)  
Gerar/atualizar features: `python3 scripts/generate_feature_docs.py`

## Regras globais (Claude Code user)

Quality / testing / security / error-observability em `~/.claude/` — aplicam a este repo.

Espelhos no repo:

- `docs/quality/test-types.md`
- `docs/quality/security-checklist.md`
- `docs/quality/error-observability.md`

## Ponteiros extras

- Test suite map: `docs/testing/types-catalog.md`
- Balance security: `docs/security/BALANCE_SECURITY.md`
- Threat model: `docs/security/threat-model-and-gaps.md`
- Deploy VM: `scripts/deploy_to_vm.py` (client default; `--backend` / `--all`)

## Armadilhas rápidas

- Admin UI **sempre pt-BR** (não i18n do app usuário)
- Nunca `useStoreA() || useStoreB()` — React #311
- HTML SPA: `Cache-Control: no-store` (`client/nginx.conf`)
- Saldos só via ledger SUM

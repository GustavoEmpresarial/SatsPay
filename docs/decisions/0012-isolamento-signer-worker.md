# ADR 0012 — Chaves de carteira só no worker (api-server watch-only)

## Status

Aceito — 2026-09-22.

## Contexto

Até aqui o bloco `&app_env` do compose entregava **o mesmo env** ao
`api-server` (exposto à internet) e ao `worker`: `ENCRYPTION_KEY`,
`HOT_MNEMONIC_ENC`, `DEPOSIT_MNEMONIC`, `HOT_WALLET_WIF`,
`HOT_WALLET_PRIVATE_KEY`, `POL_HOT_WALLET_KEY`. Problemas:

1. O `api-server` não assina nada (broadcast, sweep e swap rodam no worker),
   mas segurava todas as chaves. Um RCE/SSRF/leitura de env na API = fundos.
   Contradiz o ADR 0005 (servidor web só com xpub).
2. O mnemonic decifrado voltava ao ambiente via `std::env::set_var` e era relido
   por `ChainRegistry`, `swap.rs` e `admin.rs` — env global como canal de segredo.
3. Só `HOT_MNEMONIC` era recusado em claro em produção; os outros quatro não.
4. **SOL**: sem hot key, o mestre de depósito SOL era `sha256(prefixo || CHAIN_DEPOSIT_XPUB)`
   — dado público (ou, com a var vazia, uma constante do código-fonte). Quem tem
   o xpub deriva as chaves de depósito SOL.
5. `CHAIN_DEPOSIT_XPUB` tinha um fallback hardcoded — que nem era um xpub válido
   (checksum falha): sem a var, BTC/LTC/… falhavam ao gerar endereço.
6. Com `DEPOSIT_MNEMONIC` cada moeda usa sua conta (`m/84'/0'/0'`, `m/44'/60'/0'`…),
   então um único xpub nunca reproduziria esses endereços.

## Decisão

- **Público** (api-server e worker): `DEPOSIT_XPUB_<COIN>` (xpub de conta,
  endereço = `…/0/{index}`, idêntico ao derivado do mnemonic) e
  `HOT_ADDRESS_<COIN>`. Em produção todo `DEPOSIT_XPUB_<COIN>` é obrigatório e
  validado; `CHAIN_DEPOSIT_XPUB` único só fora de produção; sem fallback hardcoded.
- **Privado** (só worker): `*_ENC` / `*_ENC_FILE`, carregados por
  `crypto::secret_bootstrap::bootstrap_signer_secrets` e passados a
  `ChainRegistry::from_env_with_signer` como parâmetro. Nada volta ao env.
  Em produção, texto claro de qualquer um dos cinco é FATAL.
  Opcional: `WALLET_ENCRYPTION_KEY` sela os `*_ENC` com chave que a API não tem.
- **api-server**: `crypto::assert_no_signer_env()` no boot — em produção,
  qualquer var de chave no env é FATAL (`SIGNER_SECRET_IN_API`).
  `ChainRegistry::from_env` é watch-only; caminhos de assinatura retornam
  `SIGNER_NOT_AVAILABLE`.
- **Coerência**: o worker recusa subir se `DEPOSIT_XPUB_<COIN>` ≠ xpub do
  `DEPOSIT_MNEMONIC` (`CHAIN_DEPOSIT_KEY_MISMATCH`) ou `HOT_ADDRESS_<COIN>` ≠
  endereço da hot key (`HOT_ADDRESS_MISMATCH`). Assim a API nunca entrega
  endereço que o worker não consegue varrer. O mnemonic de depósito é
  obrigatório no signer de produção. Após validar, o worker publica no banco
  um heartbeat ligado ao SHA-256 da configuração pública e da rede; `/healthz`
  e toda emissão de endereço falham com 503 se o marcador divergir ou passar
  90 segundos sem renovação.
- **SOL** (ed25519 não tem derivação pública): o worker pré-deriva endereços em
  `deposit_address_pool` (migration 0036, `DEPOSIT_POOL_TARGET`, padrão 50);
  a API reivindica com `FOR UPDATE SKIP LOCKED`. Pool vazio →
  `DEPOSIT_ADDRESS_POOL_EMPTY` (503). O mestre SOL nunca vem de xpub; a fórmula
  de índices já emitidos (mnemonic ou hot key legada) não muda.
- Guardas de CI: Semgrep `wallet-key-env-outside-bootstrap` (ERROR) e checagem
  do compose (`api-server` sem vars de chave).

## Consequências

- Deploy exige migrar o `.env` (runbook `vm-security-runbook.md` §0) antes de
  subir as imagens novas; sem isso API e/ou worker recusam o boot — de propósito.
- Endereços SOL passam a depender do worker estar vivo para reabastecer o pool.
- Ainda no mesmo host: comprometer a VM inteira continua expondo o worker.
  Próximo passo (fora deste ADR): signer em host separado ou HSM/MPC, e teto de
  saldo na hot com sobra para endereço cold watch-only.

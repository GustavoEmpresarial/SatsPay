# AGENTS.md — BitcoSats

Leia primeiro: [`CLAUDE.md`](CLAUDE.md) e [`docs/features/README.md`](docs/features/README.md).

Quando precisar entender uma aba ou domínio:

```bash
rg -n "tesouraria|merchants|faucet|ledger" docs/features
```

Abra `docs/features/<slug>/FEATURE.md` e `TC.md`.

`toEmail` em `POST /v1/public/send` é o e-mail da conta que recebe: digitado ou o `email` do OAuth, o mesmo campo. Sem conta: `TARGET_INELIGIBLE`. A conta dona da chave: `SEND_TO_SELF` (não é falta de saldo). Checkout da própria fatura: `CANNOT_PAY_OWN_INVOICE`. Sempre `{ "error", "code" }`, nada debitado.

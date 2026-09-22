# API Pública com assinatura HMAC

A API Pública do **BitcoSats/SatsPay** permite que um servidor parceiro consulte saldo,
faça transferências internas e opere o gateway de cobranças com uma credencial própria,
sem sessão de usuário.

> Este documento foi reescrito contra o código (`crates/db/src/public_api.rs` e
> `crates/api-http/src/public_api.rs`). A versão anterior descrevia headers, uma string
> canônica e respostas que a API nunca implementou — se você integrou por ela, os pontos
> divergentes estão listados em [§7](#7-o-que-mudou-em-relação-à-versão-anterior-deste-guia).

---

## 1. Emitir a chave (`/v1/api-keys`)

*Aliases do mesmo handler: `/v1/public/keys`, `/public/keys`, `/api-keys`.*

### `POST /v1/api-keys`
- **Autenticação**: Bearer JWT (uma chave é emitida por uma sessão de usuário, nunca por outra chave).
- **Body**:
  ```json
  {
    "label": "Bot de Pagamento Produção",
    "scopes": ["deposits", "send"],
    "allowedIps": ["203.0.113.10", "198.51.100.0/24", "2001:db8::/32"],
    "expiresInDays": 90,
    "requireSignature": true
  }
  ```
- **Resposta `201 Created`**:
  ```json
  {
    "id": "0f2b8c1e-…",
    "key": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
    "prefix": "9f86d081"
  }
  ```
  O segredo vem em **`key`** — 64 caracteres hexadecimais — e aparece **uma única vez**.
  `id` é o que vai no header `x-key-id` do modo assinado. `prefix` serve só para você
  reconhecer a chave na listagem.

`allowedIps` aceita IP exato, CIDR IPv4/IPv6 (`198.51.100.0/24`), `*` (qualquer origem) e IPv4-mapped IPv6 (`::ffff:1.2.3.4` casa com `1.2.3.4`). Lista vazia = sem restrição de IP.

### Escopos

Apenas três valores mudam alguma coisa hoje:

| Escopo | O que libera |
|---|---|
| `deposits` | criar e consultar cobranças em `/v1/merchant/deposits` |
| `send` | `POST /v1/public/send` |
| `balance` | `GET /v1/public/balance` |
| `*` | curinga: concede tudo |

Qualquer outro texto é **recusado** na emissão (`400 INVALID_SCOPE`). Escopo ausente → `403 MISSING_SCOPE`.

### `GET /v1/api-keys`
Lista as chaves do usuário: `id`, `label`, `keyPrefix`, `scopes`, `allowedIps`, `expiresAt`,
`requireSignature`, `createdAt`, `disabledAt`, `lastUsedAt`. **O segredo não é devolvido** —
não há como recuperá-lo.

### `POST /v1/api-keys/:id/rotate`
Novo segredo, mesmo `id`, mesmos escopos, mesma allowlist, mesma expiração, mesma política
de assinatura. Resposta no mesmo formato de `IssuedKey`. **O segredo anterior para de valer
imediatamente** — implante o novo antes de rotacionar.

### `DELETE /v1/api-keys/:id`
`204 No Content`. Desativa a chave; a linha permanece para a auditoria.

---

## 2. Os dois modos de autenticação

A API pública aceita **um** dos dois, nunca ambos na mesma requisição:

### Modo simples — `x-api-key`

```http
x-api-key: 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
```

O valor tem de ter exatamente 64 caracteres hexadecimais; qualquer outra forma é recusada
sem sequer consultar o banco. Uma chave emitida com `requireSignature: true`
**rejeita este modo** — é o ponto do sinalizador.

### Modo assinado — HMAC-SHA256

| Header | Conteúdo |
|---|---|
| `x-key-id` | o **`id`** (UUID) da chave, não o segredo |
| `x-timestamp` | UNIX em **segundos ou milissegundos** (valores abaixo de `1e12` são lidos como segundos) |
| `x-signature` | HMAC-SHA256 em hexadecimal minúsculo |

Não existe header de nonce: o **anti-replay é a própria assinatura**, reservada em
Postgres como uso único (`public_api_signature_nonces`). Reenviar a mesma requisição
assinada dentro da janela responde `SIGNATURE_REPLAY`, ainda que tudo o mais esteja certo.

A defasagem máxima entre `x-timestamp` e o relógio do servidor é
`PUBLIC_API_SIGNATURE_MAX_SKEW_SECS`; fora dela, `TIMESTAMP_OUT_OF_WINDOW`.

---

## 3. A string canônica

Quatro linhas separadas por `\n`, **nesta ordem**:

```text
{TIMESTAMP}\n{METHOD}\n{PATH}\n{SHA256_HEX(BODY)}
```

- `TIMESTAMP` — exatamente o texto enviado em `x-timestamp`.
- `METHOD` — em **maiúsculas** (o servidor normaliza; assine já em maiúsculas).
- `PATH` — caminho **com a query string**, se houver: `/v1/public/balance?coin=BTC`.
- Última linha — o **SHA-256 do corpo cru em hexadecimal**, não o corpo. Para `GET` (corpo
  vazio) é o hash da string vazia:
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

A assinatura é `HMAC-SHA256(key = segredo_da_chave, data = string_canônica)`, em hexadecimal.

Como o corpo entra pelo **hash do byte-a-byte**, assine exatamente os bytes que você vai
enviar: serialize o JSON uma vez, guarde a string, e mande essa mesma string. Re-serializar
entre assinar e enviar muda a assinatura.

---

## 4. Endpoints

### `POST /v1/public/send` — transferência interna

**`toEmail`** é o e-mail da conta que recebe. Os dois jeitos abaixo são válidos e caem no mesmo campo: o usuário digita o e-mail da conta SatsPay dele, ou entra com SatsPay e você usa o `email` de `GET /v1/oauth/userinfo` (`email_verified: true`). Não é endereço on-chain.

Toda falha deste endpoint é `400` com `{ "error": "…", "code": "…" }`. Nada é debitado.

| `code` | O que aconteceu |
|---|---|
| `TARGET_INELIGIBLE` | Não existe conta SatsPay com esse e-mail. |
| `SEND_TO_SELF` | O e-mail é da conta que emitiu a chave. Não é falta de saldo. |
| `DAILY_LIMIT_REACHED` | A chave estourou o limite diário. |
| `WALLET_NOT_FOUND` | Remetente ou destinatário sem carteira nessa moeda. |

Mostre `error` e `code` para quem chama. Não troque por “tente de novo” nem por “saldo insuficiente”.

- **Escopo**: `send`.
- **Body** — só estes quatro campos são lidos:
  ```json
  {
    "coin": "USDT",
    "toEmail": "cliente@example.com",
    "amount": "2500000000",
    "idempotencyKey": "pedido-1234-reembolso"
  }
  ```
- `amount` é **inteiro em unidades de ledger (1e-8)**: 25 USDT é `"2500000000"`.
- `idempotencyKey` é **obrigatório** e escopado à chave: repetir devolve a operação
  original em vez de enviar de novo.
- Transferência **dentro da plataforma**, da carteira do dono da chave para a conta do
  e-mail informado. Não é saque on-chain.
- Há limite diário por conta (`PUBLIC_API_DAILY_SEND_LIMIT`).
- **Resposta `200`**: `{ "referenceId": "…" }`. O dono da chave recebe um e-mail avisando
  do envio.

### `GET /v1/public/balance` — saldo
- **Sem verificação de escopo** (ver o aviso em §1).
- **Não aceita filtro**: devolve **todas** as moedas, num mapa de símbolo para quantia em
  unidades de ledger.
  ```json
  { "BTC": "500000", "USDT": "2500000000", "POL": "0" }
  ```

### Gateway de cobranças
`POST /v1/merchant/deposits` e as demais rotas do gateway usam a **mesma** autenticação
(escopo `deposits`). O contrato está em [`http-api-reference.md`](http-api-reference.md) §10.

---

## 5. Exemplo completo (Python)

```python
import hashlib
import hmac
import json
import time
import uuid

import requests

KEY_ID = "0f2b8c1e-…"      # id da chave (x-key-id)
SECRET = "9f86d081…"        # segredo devolvido em `key`, uma única vez
BASE_URL = "https://www.satspay.pro"
PATH = "/v1/public/send"
METHOD = "POST"

body = json.dumps(
    {
        "coin": "USDT",
        "toEmail": "destinatario@example.com",
        "amount": "2500000000",     # 25 USDT em unidades de 1e-8
        "idempotencyKey": str(uuid.uuid4()),
    },
    separators=(",", ":"),
).encode()

timestamp = str(int(time.time()))
body_hash = hashlib.sha256(body).hexdigest()
canonical = f"{timestamp}\n{METHOD}\n{PATH}\n{body_hash}"
signature = hmac.new(SECRET.encode(), canonical.encode(), hashlib.sha256).hexdigest()

res = requests.post(
    f"{BASE_URL}{PATH}",
    data=body,                      # os MESMOS bytes que foram assinados
    headers={
        "Content-Type": "application/json",
        "x-key-id": KEY_ID,
        "x-timestamp": timestamp,
        "x-signature": signature,
    },
)
print(res.status_code, res.json())
```

### Node.js

```js
import { createHash, createHmac } from 'node:crypto';

const body = JSON.stringify({
  coin: 'USDT',
  toEmail: 'destinatario@example.com',
  amount: '2500000000',
  idempotencyKey: crypto.randomUUID(),
});

const timestamp = String(Math.floor(Date.now() / 1000));
const bodyHash = createHash('sha256').update(body).digest('hex');
const canonical = `${timestamp}\nPOST\n/v1/public/send\n${bodyHash}`;
const signature = createHmac('sha256', SECRET).update(canonical).digest('hex');

await fetch('https://www.satspay.pro/v1/public/send', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'x-key-id': KEY_ID,
    'x-timestamp': timestamp,
    'x-signature': signature,
  },
  body, // a mesma string assinada
});
```

### Erros de autenticação

| `code` | Significado |
|---|---|
| `INVALID_API_KEY` | header ausente, ou não são 64 hex |
| `INVALID_KEY_ID` | `x-key-id` não é um UUID |
| `KEY_NOT_FOUND` | nenhuma chave com aquele id |
| `SIGNATURE_MISMATCH` | a assinatura não confere — quase sempre a string canônica |
| `SIGNATURE_REPLAY` | assinatura já usada dentro da janela |
| `TIMESTAMP_OUT_OF_WINDOW` | relógio fora da defasagem aceita |
| `MISSING_SCOPE` | a chave não tem o escopo daquela rota |

Chave desativada, expirada ou usada de um IP fora da allowlist também falha aqui, sem
distinguir o motivo na resposta.

---

## 6. Webhooks de fatura (`X-SatsPay-Signature`)

Chave **diferente** e algoritmo **diferente** do acima — não reaproveite o código de §3.

Quando uma fatura é confirmada, o gateway faz `POST` na `callbackUrl` com:

```http
X-SatsPay-Signature: sha256=<hex>
X-SatsPay-Event: deposit.confirmed
X-SatsPay-Timestamp: 1788278400
X-SatsPay-Delivery: <uuid>
```

A verificação é `HMAC-SHA256(key = signing_secret, data = corpo_cru)` — o corpo inteiro,
não o seu hash, e nada de método, caminho ou timestamp. Compare em tempo constante com o
valor após `sha256=`.

O segredo **não** é `satspay_secret_default`. Cada comerciante tem um segredo hex estável
derivado de `ENCRYPTION_KEY` (HKDF `bitcosats:webhook:v1`). Obtenha-o autenticado:

```http
GET /v1/merchant/webhook-signing-secret
Authorization: Bearer <accessToken>
```

Faturas do mesmo comerciante compartilham a chave; comerciantes diferentes têm chaves
diferentes. O `timestamp` também vai **dentro** do corpo assinado, para você recusar replay
fora de 300s sem confiar num header não assinado.

Para testar sua verificação sem esperar um pagamento real:
`POST /v1/merchant/deposits/:id/test-webhook` (Bearer JWT) envia uma entrega assinada igual
à verdadeira.

---

## 7. O que mudou em relação à versão anterior deste guia

A versão publicada antes descrevia um protocolo que a API nunca implementou. Se você
integrou por ela, revise:

| A doc antiga dizia | O código faz |
|---|---|
| headers `X-Api-Key`, `X-Timestamp`, `X-Nonce`, `X-Signature` | modo assinado usa `x-key-id`, `x-timestamp`, `x-signature`; **não existe `X-Nonce`** |
| string canônica `MÉTODO\nCAMINHO\nTS\nNONCE\nCORPO` | `TS\nMÉTODO\nCAMINHO\nSHA256(CORPO)` — outra ordem, e o **hash** do corpo |
| chave emitida em `apiKey` | o campo é **`key`** |
| escopos incluíam `balance` mas a doc antiga não batia com o código | `GET /v1/public/balance` exige o escopo `balance` (`403 MISSING_SCOPE` se faltar). `*` concede tudo |
| `send` aceitava `memo` | campo inexistente, ignorado |
| `send` respondia `{success, transactionId, coin, amount}` | responde `{ "referenceId": "…" }` |
| `balance` aceitava `?coin=BTC` e devolvia uma moeda | ignora a query e devolve **todas** as moedas |
| `send` só devolvia `{ "error": "cannot send to self" }` | `400` `{ "error", "code": "SEND_TO_SELF" }`. A conta dona da chave não recebe o próprio envio. Checkout com saldo da mesma conta: `CANNOT_PAY_OWN_INVOICE` |
| exemplo apontava para `http://127.0.0.1:4000` | `https://www.satspay.pro` |

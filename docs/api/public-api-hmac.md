# Guia de Integração da API Pública com Assinatura HMAC

A API Pública do **BitcoSats** permite que parceiros, comerciantes e sistemas automatizados realizem transferências internas e consultem saldos programaticamente com alta segurança.

---

## 1. Gestão de Chaves de API (`/v1/api-keys`)

Antes de realizar chamadas, o desenvolvedor deve emitir uma chave de API através do painel ou dos endpoints autenticados:

### `POST /v1/api-keys`
- **Autenticação**: Bearer JWT
- **Body**:
  ```json
  {
    "label": "Bot de Pagamento Produção",
    "scopes": ["SEND", "BALANCE"],
    "allowedIps": ["203.0.113.10", "198.51.100.25"],
    "expiresInDays": 90,
    "requireSignature": true
  }
  ```
- **Resposta `201 Created`**:
  ```json
  {
    "id": "uuid",
    "apiKey": "bts_live_a1b2c3d4e5f6g7h8i9j0...",
    "label": "Bot de Pagamento Produção",
    "scopes": ["SEND", "BALANCE"],
    "requireSignature": true
  }
  ```
  > **Atenção**: O valor completo da chave de API (`apiKey`) é exibido apenas uma única vez na criação. Guarde-o com segurança.

---

## 2. Protocolo de Autenticação e Assinatura HMAC-SHA256

Quando `requireSignature = true`, todas as requisições para a API pública devem conter os seguintes headers HTTP:

| Header | Descrição | Exemplo |
|---|---|---|
| `X-Api-Key` | A chave de API do desenvolvedor | `bts_live_...` |
| `X-Timestamp` | Timestamp UNIX em segundos (máximo de 300s de defasagem permitida) | `1788278400` |
| `X-Nonce` | Identificador aleatório único para prevenir ataques de repetição (*replay attacks*) | `c7a1f2e8-45d2...` |
| `X-Signature` | Assinatura HMAC-SHA256 em hexadecimal | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

---

## 3. Algoritmo de Cálculo da Assinatura

A string de pré-assinatura (*payload string*) deve ser montada concatenando os seguintes elementos separados por quebras de linha (`\n`):

```text
{HTTP_METHOD}\n
{REQUEST_PATH}\n
{TIMESTAMP}\n
{NONCE}\n
{REQUEST_BODY}
```

Onde:
- `HTTP_METHOD`: `POST`, `GET`, etc. (em letras maiúsculas).
- `REQUEST_PATH`: Caminho da rota (ex: `/v1/public/send`).
- `TIMESTAMP`: O mesmo valor enviado no header `X-Timestamp`.
- `NONCE`: O mesmo valor enviado no header `X-Nonce`.
- `REQUEST_BODY`: O corpo exato da requisição em JSON (string vazia `""` para requisições `GET`).

A assinatura é calculada como:
$$\text{Signature} = \text{HMAC-SHA256}(\text{key} = \text{apiKey}, \text{data} = \text{payload\_string})$$

---

## 4. Endpoints da API Pública (`/v1/public`)

### 4.1 Enviar Fundos (`POST /v1/public/send`)
- **Headers**: `X-Api-Key`, `X-Timestamp`, `X-Nonce`, `X-Signature` (se exigido).
- **Body**:
  ```json
  {
    "toEmail": "cliente@example.com",
    "coin": "BTC",
    "amount": "25000",
    "memo": "Reembolso Pedido #1234",
    "idempotencyKey": "pedido-1234-reembolso"
  }
  ```
- **Resposta `200 OK`**:
  ```json
  {
    "success": true,
    "transactionId": "uuid",
    "coin": "BTC",
    "amount": "25000"
  }
  ```

### 4.2 Consultar Saldo (`GET /v1/public/balance`)
- **Headers**: `X-Api-Key`, `X-Timestamp`, `X-Nonce`, `X-Signature`.
- **Query Params**: `?coin=BTC`
- **Resposta `200 OK`**:
  ```json
  {
    "coin": "BTC",
    "balance": "500000"
  }
  ```

---

## 5. Exemplo de Implementação em Python

```python
import hmac
import hashlib
import time
import uuid
import requests
import json

API_KEY = "bts_live_sua_chave_aqui"
BASE_URL = "http://127.0.0.1:4000"
PATH = "/v1/public/send"
METHOD = "POST"

body_data = {
    "toEmail": "destinatario@example.com",
    "coin": "BTC",
    "amount": "10000",
    "memo": "Transferencia API",
    "idempotencyKey": str(uuid.uuid4())
}
body_json = json.dumps(body_data, separators=(',', ':'))

timestamp = str(int(time.time()))
nonce = str(uuid.uuid4())

# Montagem do payload de assinatura
payload = f"{METHOD}\n{PATH}\n{timestamp}\n{nonce}\n{body_json}"
signature = hmac.new(API_KEY.encode('utf-8'), payload.encode('utf-8'), hashlib.sha256).hexdigest()

headers = {
    "Content-Type": "application/json",
    "X-Api-Key": API_KEY,
    "X-Timestamp": timestamp,
    "X-Nonce": nonce,
    "X-Signature": signature
}

response = requests.post(f"{BASE_URL}{PATH}", data=body_json, headers=headers)
print(response.status_code, response.json())
```

---

## 6. Webhooks de invoice (`X-SatsPay-Signature`)

Quando uma invoice é confirmada, o gateway POSTa o JSON em `callback_url` com:

```http
X-SatsPay-Signature: sha256=<hex>
```

A chave **não** é `satspay_secret_default`. Cada merchant tem um segredo hex estável derivado de `ENCRYPTION_KEY` (HKDF `bitcosats:webhook:v1`). Obtenha o valor autenticado:

```http
GET /v1/merchant/webhook-signing-secret
Authorization: Bearer <accessToken>
```

Verificação (mesmo algoritmo da API pública, chave diferente):

```text
HMAC-SHA256(key = signing_secret, data = raw_request_body)
```

Compare o hex com o valor após `sha256=` no header (comparação em tempo constante). Invoices do mesmo merchant compartilham a chave; merchants diferentes têm chaves diferentes.

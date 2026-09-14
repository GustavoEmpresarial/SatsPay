# Sistema de Detecção, Coleta, Classificação e Observabilidade de Erros

Uma camada mais ampla que testes e vulnerabilidades: um sistema completo de **detecção, coleta, classificação, rastreamento e observabilidade de erros**.

## 1. Tipos de erros

### Erros de aplicação

- `ValidationError`
- `AuthenticationError`
- `AuthorizationError`
- `NotFoundError`
- `ConflictError`
- `RateLimitError`
- `BusinessRuleError`
- `InternalServerError`
- `ConfigurationError`
- `DependencyError`

### Erros de infraestrutura

- Database connection error
- Database timeout
- Redis connection error
- Queue failure
- Storage failure
- Network error
- DNS error
- TLS/SSL error
- Container failure
- Memory exhaustion
- CPU exhaustion
- Disk exhaustion

### Erros externos

- API externa indisponível
- API externa retornando 4xx
- API externa retornando 5xx
- Timeout
- Invalid response
- Schema incompatível
- Rate limit externo
- Serviço externo alterado

### Erros de dados

- Invalid data
- Missing data
- Corrupted data
- Duplicate data
- Inconsistent data
- Constraint violation
- Serialization error
- Deserialization error
- Invalid JSON
- Invalid type

### Erros de negócio

- Saldo insuficiente
- Operação duplicada
- Estado inválido
- Limite excedido
- Recompensa duplicada
- Saque inválido
- Depósito não reconciliado
- Máquina inexistente
- Máquina congelada
- Operação não permitida

---

## 2. Classificação por severidade

| Nível | Significado |
|---|---|
| `DEBUG` | Informação para desenvolvimento |
| `INFO` | Operação normal |
| `NOTICE` | Situação que merece atenção |
| `WARNING` | Problema potencial |
| `ERROR` | Operação falhou |
| `CRITICAL` | Falha grave |
| `FATAL` | Aplicação/serviço não consegue continuar |

Exemplo:

```
INFO
Usuário realizou login

WARNING
API externa demorou 4.8s

ERROR
Falha ao processar pagamento

CRITICAL
Ledger inconsistente

FATAL
Banco de dados indisponível
```

---

## 3. Classificação por origem

Todo erro deveria possuir uma origem:

```
CLIENT
API
AUTH
DATABASE
CACHE
QUEUE
WORKER
CRON
EXTERNAL_API
BLOCKCHAIN
PAYMENT
STORAGE
INFRASTRUCTURE
SECURITY
BUSINESS
UNKNOWN
```

Isso permite descobrir rapidamente: "De onde estão vindo os problemas?"

---

## 4. Classificação por impacto

Além da severidade:

```
LOW
MEDIUM
HIGH
CRITICAL
```

Vale separar **severity** de **impact**.

Exemplo:

```
severity = ERROR
impact = LOW
```

Uma busca que falhou pode ser ERROR, mas não necessariamente derruba o sistema.

Já:

```
severity = CRITICAL
impact = CRITICAL
```

pode representar corrupção de saldo.

---

## 5. Error Code

Não depender apenas da mensagem.

Em vez de:

```
"Erro ao processar saque"
```

usar:

```json
{
  "code": "WITHDRAWAL_PROCESSING_FAILED"
}
```

Exemplos:

```
AUTH_INVALID_CREDENTIALS
AUTH_TOKEN_EXPIRED
AUTH_FORBIDDEN

USER_NOT_FOUND
USER_ALREADY_EXISTS

WALLET_NOT_FOUND
WALLET_INSUFFICIENT_BALANCE

WITHDRAWAL_DUPLICATE
WITHDRAWAL_INVALID_STATE
WITHDRAWAL_PROCESSING_FAILED

DATABASE_CONNECTION_FAILED
DATABASE_TIMEOUT

EXTERNAL_API_TIMEOUT
EXTERNAL_API_INVALID_RESPONSE

BLOCKCHAIN_RPC_ERROR
BLOCKCHAIN_TRANSACTION_FAILED
BLOCKCHAIN_CONFIRMATION_TIMEOUT
```

---

## 6. Error ID

Cada ocorrência deve possuir um ID único.

```
error_id = err_01J...
```

Isso permite:

```
Usuário
 ↓
erro exibido
 ↓
error_id
 ↓
logs
 ↓
stack trace
 ↓
request
 ↓
database
```

O suporte pode pedir: "Me manda o código do erro." E encontrar exatamente aquela ocorrência.

---

## 7. Request ID / Correlation ID

Extremamente importante.

Uma requisição:

```
Request
   ↓
API
   ↓
Service
   ↓
Database
   ↓
Queue
   ↓
Worker
   ↓
External API
```

deve carregar:

```
correlation_id
```

Exemplo:

```
correlation_id = req_abc123
```

Assim é possível reconstruir toda a operação.

---

## 8. Stack Trace

Todo erro inesperado deve coletar:

```
Error
Message
Stack
File
Line
Column
Function
Module
```

Exemplo:

```
TypeError: Cannot read properties of undefined
    at WalletService.transfer()
    at WalletController.transfer()
```

---

## 9. Contexto do erro

Um erro sem contexto é quase inútil.

Coletar:

```json
{
  "error": "...",
  "code": "...",
  "severity": "ERROR",
  "module": "wallet",
  "operation": "transfer",
  "request_id": "...",
  "user_id": "...",
  "environment": "production",
  "version": "1.8.2"
}
```

Mas **não coletar secrets**.

Nunca registrar:

```
password
private_key
seed_phrase
JWT
API_KEY
credit_card
2FA secret
```

---

## 10. HTTP Error Collection

Capturar automaticamente:

```
400
401
403
404
409
422
429
500
502
503
504
```

E registrar:

```
method
path
status
duration
request_id
error_code
```

Exemplo:

```
POST /api/withdraw
→ 500
→ 842ms
→ WITHDRAWAL_PROCESSING_FAILED
```

---

## 11. Frontend Error Collection

O frontend também deve enviar erros.

Coletar:

- JavaScript errors
- Promise rejection
- API errors
- Network errors
- Component errors
- Rendering errors
- Chunk loading errors
- WebSocket errors

Exemplo:

```
Frontend
   ↓
Error Collector
   ↓
POST /internal/errors
   ↓
Backend
```

---

## 12. Backend Error Collection

Capturar globalmente:

```
Unhandled exception
Unhandled rejection
HTTP errors
Database errors
Worker errors
Queue errors
Cron errors
External API errors
```

Em Node:

```
uncaughtException
unhandledRejection
```

Mas esses eventos devem ser tratados como **falhas graves**, não simplesmente "capturados e ignorados". Dependendo do estado do processo, o serviço pode precisar reiniciar.

---

## 13. Worker Error Collection

Workers precisam de coleta própria.

```
Worker
 ↓
Job
 ↓
Error
 ↓
Retry
 ↓
Retry
 ↓
Retry
 ↓
Dead Letter Queue
```

Registrar:

```
job_id
job_type
attempt
max_attempts
error_code
duration
```

---

## 14. Cron Error Collection

Para cada cron:

```
cron_id
job
started_at
finished_at
duration
status
error
```

Exemplo:

```
reward_distribution
STARTED 21:00
FINISHED 21:03
STATUS SUCCESS
```

ou:

```
reward_distribution
STARTED 21:00
FAILED 21:01
ERROR DATABASE_TIMEOUT
```

---

## 15. Database Error Collection

Detectar:

- Connection failure
- Connection timeout
- Query timeout
- Deadlock
- Constraint violation
- Unique violation
- Foreign key violation
- Transaction rollback
- Pool exhaustion
- Migration failure

---

## 16. External API Error Collection

Toda integração deveria ter:

```
provider
endpoint
status
duration
timeout
retry_count
response_code
error_code
```

Exemplo:

```
provider = Binance
operation = getPrice
status = TIMEOUT
duration = 5000ms
retry = 3
```

---

## 17. Blockchain Error Collection

Para Web3:

```
RPC error
Transaction rejected
Transaction reverted
Nonce error
Gas estimation failure
Insufficient gas
Confirmation timeout
Reorg
Invalid transaction
Invalid signature
Wrong chain
Provider unavailable
```

Coletar também:

```
chain
network
tx_hash
wallet_address
block_number
confirmation_count
rpc_provider
```

**Nunca coletar private key/seed.**

---

## 18. Erros de segurança

Separar erros normais de eventos de segurança.

Exemplos:

```
INVALID_TOKEN
AUTH_FAILURE
RATE_LIMIT_EXCEEDED
PERMISSION_DENIED
SUSPICIOUS_REQUEST
IDOR_ATTEMPT
INVALID_SIGNATURE
REPLAY_DETECTED
```

Esses eventos podem alimentar um sistema de detecção.

---

## 19. Detecção automática

Não basta armazenar erros. O sistema deve procurar padrões.

Exemplo:

```
1 erro
 ↓
normal

10 erros/min
 ↓
WARNING

100 erros/min
 ↓
CRITICAL

1000 erros/min
 ↓
INCIDENT
```

---

## 20. Error Grouping

Muito importante.

Imagine:

```
10.000 ocorrências
```

mas todas são:

```
TypeError: Cannot read properties of undefined
```

Não é desejável ter 10.000 problemas separados.

Agrupar por:

```
error fingerprint
```

Exemplo:

```
GROUP-001
TypeError
WalletService.transfer
line 87
```

Resultado:

```
Occurrences: 10.284
Users affected: 2.391
First seen: 10:32
Last seen: 14:51
```

---

## 21. Fingerprint

Criar uma assinatura baseada em:

```
error type
error code
module
function
stack trace
```

Assim:

```
ERROR A
ERROR A
ERROR A
```

vira um único grupo.

---

## 22. Deduplicação

Evitar spam.

Por exemplo:

```
Banco caiu
 ↓
100.000 requests
 ↓
100.000 erros
```

Não mandar 100.000 notificações. Em vez disso:

```
DATABASE_CONNECTION_FAILED

Occurrences: 100000
First: 14:00
Last: 14:05
```

---

## 23. Error Rate

Monitorar:

```
errors/min
errors/hour
errors/day
```

E principalmente:

```
error_rate / total_requests
```

Exemplo:

```
100.000 requests
500 errors

error rate = 0,5%
```

---

## 24. Error Budget

Em sistemas mais maduros, usar:

```
SLO
SLA
SLI
Error Budget
```

Exemplo:

```
SLO = 99,9%

Error Budget mensal:
≈ 43 minutos
```

---

## 25. Alertas automáticos

O sistema pode gerar:

```
WARNING
ERROR SPIKE
CRITICAL
SERVICE DOWN
DATABASE DOWN
```

Enviar para: Discord, Telegram, Email, Slack, PagerDuty, Dashboard

---

## 26. Alertas inteligentes

Não alertar simplesmente:

```
Erro ocorreu.
```

Melhor:

```
🚨 ERROR SPIKE

Service: API
Module: Wallet
Error: WITHDRAWAL_PROCESSING_FAILED

Current: 184 errors/min
Normal: 2 errors/min

Affected users: 73
Started: 14:31

Version: 2.4.1
```

---

## 27. Health Checks

Criar:

```
/health
```

e, idealmente, separar:

```
/liveness
/readiness
```

Exemplo:

```
API
 ├── Database
 ├── Redis
 ├── Queue
 ├── External API
 └── Blockchain RPC
```

---

## 28. Dependency Health

Monitorar automaticamente:

```
PostgreSQL
Redis
RabbitMQ/Kafka
Blockchain RPC
Storage
External APIs
```

Resultado:

```
DATABASE      🟢
REDIS         🟢
QUEUE         🟢
BLOCKCHAIN    🟡
PAYMENT API   🔴
```

---

## 29. Performance Errors

Nem todo problema é uma exceção.

Detectar:

```
Slow request
Slow query
Slow external API
Slow job
Slow queue
High memory
High CPU
```

Exemplo:

```
GET /dashboard

Normal: 180ms
Current: 4.8s

→ PERFORMANCE_DEGRADATION
```

---

## 30. Memory / Resource Errors

Monitorar:

- RAM
- CPU
- Disk
- Network
- DB connections
- Redis memory
- Queue size
- Open files
- Threads
- Event loop lag

---

## 31. Error Lifecycle

Cada erro pode ter estado:

```
NEW
 ↓
ACKNOWLEDGED
 ↓
INVESTIGATING
 ↓
FIXED
 ↓
RESOLVED
```

Ou:

```
IGNORED
DUPLICATE
WONT_FIX
EXPECTED
```

---

## 32. Classificação automática

O sistema pode classificar:

```
TYPE
SEVERITY
CATEGORY
SOURCE
MODULE
ENVIRONMENT
```

Exemplo:

```json
{
  "type": "DatabaseError",
  "category": "DATABASE",
  "severity": "CRITICAL",
  "source": "backend",
  "module": "wallet",
  "environment": "production"
}
```

---

## 33. Tags

Adicionar tags:

```
module=wallet
service=api
environment=production
version=2.4.1
provider=polygon
operation=withdraw
```

Isso facilita filtros.

---

## 34. Breadcrumbs

Antes do erro:

```
LOGIN
 ↓
OPEN_WALLET
 ↓
GET_BALANCE
 ↓
CLICK_WITHDRAW
 ↓
CREATE_WITHDRAWAL
 ↓
ERROR
```

Isso ajuda muito a reconstruir o que aconteceu.

---

## 35. Distributed Tracing

Em arquitetura distribuída:

```
Request
 │
 ├── API
 │
 ├── Auth
 │
 ├── Wallet
 │
 ├── Database
 │
 └── Worker
```

Cada etapa recebe um trace/span. É possível descobrir: "A requisição demorou 4 segundos porque o PostgreSQL levou 3,7 segundos."

---

## 36. Métricas

Não coletar apenas logs.

Ter:

```
Logs
Metrics
Traces
Errors
Events
```

Essa é a base de observabilidade moderna.

---

## 37. Dashboard de erros

Exemplo:

```
┌─────────────────────────────────────────┐
│ ERROR MONITOR                            │
├─────────────────────────────────────────┤
│ Errors today             12,481          │
│ Error rate                 0.43%         │
│ Critical                     12          │
│ Affected users            1,284          │
├─────────────────────────────────────────┤
│ TOP ERRORS                              │
│                                         │
│ DATABASE_TIMEOUT          4,821          │
│ API_TIMEOUT               2,381          │
│ INVALID_STATE             1,992          │
│ WITHDRAWAL_FAILED         1,203          │
└─────────────────────────────────────────┘
```

---

## 38. Dashboard por módulo

```
AUTH       12 errors
USERS      2 errors
WALLET     843 errors
PAYMENTS   31 errors
MINING     1,203 errors
BLOCKCHAIN 92 errors
ADMIN      4 errors
```

---

## 39. Dashboard por versão

Muito útil depois de deploy.

```
v2.3.0 → 0.12%
v2.3.1 → 0.15%
v2.4.0 → 4.82% 🚨
```

Descoberta imediata: o novo deploy aumentou os erros.

---

## 40. Deploy correlation

Relacionar:

```
DEPLOY
 ↓
ERROR SPIKE
```

Exemplo:

```
14:00 → deploy v2.5.0
14:04 → errors começam
14:08 → error rate 8%
```

Isso é excelente para detectar regressões.

---

## 41. Error Collection Architecture

Arquitetura sugerida:

```
                    ┌─────────────┐
                    │   Browser   │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ Error SDK   │
                    └──────┬──────┘
                           │
┌─────────┐         ┌──────▼──────┐
│ Backend │────────►│ Error       │
├─────────┤         │ Collector   │
│ Workers │────────►│             │
├─────────┤         └──────┬──────┘
│ Cron    │────────►       │
└─────────┘                │
                           ▼
                    ┌─────────────┐
                    │ Classifier  │
                    └──────┬──────┘
                           │
             ┌─────────────┼─────────────┐
             ▼             ▼             ▼
          Storage        Metrics       Alerts
             │             │             │
             ▼             ▼             ▼
         Dashboard      Grafana       Discord
```

---

## 42. Estrutura sugerida no projeto

Seguindo a ideia de **modular monolith**, evitar criar um sistema gigante espalhado pelo projeto.

```
server/
├── core/
│   ├── errors/
│   │   ├── app-error.ts
│   │   ├── error-codes.ts
│   │   ├── error-classifier.ts
│   │   ├── error-handler.ts
│   │   ├── error-context.ts
│   │   ├── error-fingerprint.ts
│   │   └── index.ts
│   │
│   ├── observability/
│   │   ├── logger.ts
│   │   ├── metrics.ts
│   │   ├── tracing.ts
│   │   └── index.ts
│   │
│   └── health/
│       ├── health-check.ts
│       ├── readiness.ts
│       ├── liveness.ts
│       └── index.ts
│
├── modules/
│   ├── auth/
│   ├── users/
│   ├── wallet/
│   ├── payments/
│   └── mining/
│
├── workers/
├── cron/
└── bootstrap/
```

---

## 43. Estrutura do registro de erro

Exemplo:

```json
{
  "id": "err_01JXYZ",
  "fingerprint": "fp_8A91",
  "timestamp": "2026-09-11T13:42:21Z",

  "type": "DatabaseError",
  "code": "DATABASE_TIMEOUT",

  "severity": "ERROR",
  "category": "DATABASE",
  "source": "backend",

  "service": "api",
  "module": "wallet",
  "operation": "transfer",

  "environment": "production",
  "version": "2.4.1",

  "request_id": "req_123",
  "trace_id": "trace_456",

  "user_id": "user_789",

  "message": "Database query timeout",

  "duration_ms": 5032
}
```

Separar **dados técnicos** de **dados sensíveis**.

---

## 44. O que NÃO deve ser coletado

Isso é tão importante quanto coletar erros.

Nunca colocar no log:

```
password
password_hash
JWT completo
refresh_token
API key
private key
seed phrase
2FA secret
cartão
CVV
session cookie
```

E dados pessoais devem ser minimizados/redigidos quando não forem necessários.

Exemplo:

```
Authorization: Bearer eyJ...
```

❌

Melhor:

```
Authorization: [REDACTED]
```

---

## 45. Sistema completo

No final, o projeto teria algo parecido com:

```
                 APPLICATION
                      │
        ┌─────────────┼─────────────┐
        ▼             ▼             ▼
      Logs          Metrics       Traces
        │             │             │
        └─────────────┼─────────────┘
                      ▼
                ERROR SYSTEM
                      │
          ┌───────────┼───────────┐
          ▼           ▼           ▼
      Collector   Classifier   Fingerprint
          │           │           │
          └───────────┼───────────┘
                      ▼
                   Storage
                      │
             ┌────────┼────────┐
             ▼        ▼        ▼
          Search   Dashboard  Alerts
                       │
              ┌────────┼────────┐
              ▼        ▼        ▼
           Discord  Telegram   Email
```

### Em termos de "camadas", o sistema se fecharia com:

1. **Error Handling** — não deixar exceções soltas.
2. **Error Collection** — coletar automaticamente.
3. **Error Classification** — determinar tipo/categoria/severidade.
4. **Error Fingerprinting** — agrupar erros iguais.
5. **Error Storage** — preservar histórico.
6. **Logging** — registrar contexto técnico.
7. **Metrics** — medir frequência e impacto.
8. **Tracing** — reconstruir o caminho da operação.
9. **Health Checks** — saber se dependências estão vivas.
10. **Alerting** — avisar automaticamente.
11. **Dashboard** — visualizar.
12. **Incident Management** — acompanhar até resolução.
13. **Security/Redaction** — impedir vazamento de informações sensíveis.
14. **Recovery** — retry, fallback, circuit breaker e recuperação.
15. **Post-mortem** — aprender com incidentes recorrentes.

Isso transforma o projeto de simplesmente "mostrar erro 500" em um sistema que consegue responder: o que aconteceu, onde aconteceu, por que aconteceu, quantas vezes aconteceu, quem foi afetado, quando começou, qual versão introduziu o problema e se já foi resolvido. ele sempre tem que trabalhar com isso, toda pagina, modulo, ou comando que eu der ele tem que trabalhar com isso sempre
</user_query>

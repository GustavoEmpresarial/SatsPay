# Tipos de Testes de Software

Listagem abrangente dos tipos de testes usados em projetos de software, organizada por nível, objetivo e característica.

## 1. Testes unitários — Unit Tests

Testam a menor unidade isolável do código, normalmente uma função, método ou classe.

- Teste de função
- Teste de método
- Teste de classe
- Teste de componente isolado
- Teste de regras de negócio
- Teste de validações
- Teste de cálculos
- Teste de branches/condições
- Teste de exceções/erros
- Teste de mocks
- Teste de stubs
- Teste de spies

**Exemplo:** verificar se `calculateReward()` retorna corretamente a recompensa de uma máquina.

---

## 2. Testes de integração — Integration Tests

Verificam se **duas ou mais partes do sistema funcionam corretamente juntas**.

- API + banco de dados
- Service + repository
- Backend + Redis
- Backend + fila
- API + serviço externo
- Autenticação + banco
- Pagamento + gateway
- Webhook + backend
- Integração entre módulos
- Integração com blockchain
- Integração com carteira
- Integração com APIs de terceiros

**Exemplo:** criar uma transação pela API e verificar se ela foi corretamente persistida no PostgreSQL.

---

## 3. Testes de sistema — System Tests

Testam o **sistema completo**, normalmente como um usuário ou consumidor externo faria.

- Fluxo completo de cadastro
- Login
- Compra
- Depósito
- Saque
- Criação de pedido
- Operação completa de uma funcionalidade
- Fluxos administrativos
- Fluxos de usuário
- Integração frontend + backend + banco

**Exemplo:**

```
Cadastro
   ↓
Login
   ↓
Depositar
   ↓
Comprar máquina
   ↓
Ativar máquina
   ↓
Receber recompensa
```

---

## 4. Testes end-to-end — E2E

Simulam o comportamento real do usuário do começo ao fim.

Normalmente usando ferramentas como Playwright, Cypress ou Selenium.

- Cadastro E2E
- Login E2E
- Checkout E2E
- Pagamento E2E
- Dashboard E2E
- Fluxos administrativos E2E
- Fluxos críticos de negócio
- Navegação completa
- Formulários
- Upload
- Download
- Notificações

**Diferença importante:**

> System Test = valida o sistema completo.
> E2E = normalmente reproduz um fluxo real completo através da interface/API.

---

## 5. Testes de API

Focados exclusivamente nas APIs.

- GET
- POST
- PUT
- PATCH
- DELETE
- Status codes
- Headers
- Authentication
- Authorization
- Payload
- Schema
- Validação de parâmetros
- Validação de body
- Paginação
- Filtros
- Ordenação
- Rate limiting
- Idempotência
- Webhooks
- Uploads
- Downloads

### Testes negativos

- Campo obrigatório ausente
- Tipo inválido
- ID inexistente
- Token inválido
- Token expirado
- Usuário sem permissão
- Payload malformado
- Valores fora do limite

---

## 6. Testes de contrato — Contract Tests

Garantem que dois sistemas continuam respeitando o **contrato de comunicação**.

Muito usados em:

```
Frontend ↔ Backend
Backend ↔ Serviço externo
Microserviço A ↔ Microserviço B
API ↔ API
```

Tipos:

- Consumer-driven contract testing
- Provider contract testing
- Schema validation
- OpenAPI contract testing
- Event contract testing

---

## 7. Testes funcionais

Verificam se uma funcionalidade faz aquilo que deveria fazer.

- Cadastro
- Login
- Recuperação de senha
- CRUD
- Busca
- Filtros
- Paginação
- Upload
- Notificações
- Pagamentos
- Relatórios
- Permissões
- Regras de negócio

Basicamente: **"A funcionalidade funciona conforme o requisito?"**

---

## 8. Testes não funcionais

Verificam **como** o sistema se comporta.

- Performance
- Segurança
- Escalabilidade
- Disponibilidade
- Confiabilidade
- Usabilidade
- Acessibilidade
- Compatibilidade
- Recuperação
- Observabilidade
- Manutenibilidade

---

## 9. Testes de performance

Avaliam velocidade, capacidade e comportamento sob carga.

### Load Test
Carga esperada.
```
100 usuários
500 usuários
1.000 usuários
```

### Stress Test
Vai além da capacidade normal.
```
1k → 5k → 10k → 50k usuários
```
Até descobrir o limite.

### Spike Test
Aumento repentino de carga.
```
100 usuários
      ↓
10.000 usuários
```

### Soak / Endurance Test
Carga prolongada.
```
1000 usuários
por 6 horas
```

### Volume Test
Grande quantidade de dados.
```
100 milhões de registros
```

### Scalability Test
Verifica se aumentar recursos aumenta a capacidade.

---

## 10. Testes de segurança

Uma categoria enorme.

- Authentication testing
- Authorization testing
- RBAC testing
- Session testing
- JWT testing
- OAuth testing
- Input validation
- SQL Injection
- XSS
- CSRF
- SSRF
- Path Traversal
- Command Injection
- Brute-force
- Rate-limit bypass
- Privilege escalation
- IDOR/BOLA
- API security
- File upload security
- Secrets exposure
- Dependency vulnerabilities
- Security headers
- CORS
- Cookie security
- Encryption testing

Também existem:

- **SAST** — Análise estática do código
- **DAST** — Testa a aplicação rodando
- **SCA** — Analisa dependências
- **Penetration Testing** — Teste de invasão autorizado

---

## 11. Testes de banco de dados

- CRUD
- Constraints
- Foreign keys
- Unique constraints
- Indexes
- Transactions
- Rollback
- Migrations
- Seed
- Referential integrity
- Concurrency
- Deadlocks
- Queries
- Performance de queries
- Data consistency
- Soft delete
- Backup/restore

---

## 12. Testes de regressão

Verificam se uma alteração **quebrou algo que já funcionava**.

Exemplo: você altera o sistema de login. A regressão verifica:

```
Login
Cadastro
Logout
Recuperação de senha
2FA
Sessão
Permissões
Admin
```

---

## 13. Smoke Tests

São testes rápidos para verificar se a aplicação está **minimamente funcional**.

```
Servidor iniciou?
    ↓
Banco conecta?
    ↓
API responde?
    ↓
Login funciona?
    ↓
Dashboard abre?
```

Se o Smoke Test falhar, normalmente nem vale a pena executar a suíte completa.

---

## 14. Sanity Tests

Parecidos com Smoke, mas mais focados em uma **alteração específica**.

Exemplo: você corrigiu o sistema de saque.

```
Criar saque
↓
Validar saldo
↓
Validar 2FA
↓
Processar saque
↓
Atualizar saldo
```

---

## 15. Testes de aceitação — Acceptance Testing

Verificam se o sistema atende aos requisitos do negócio.

- UAT — User Acceptance Testing
- Business Acceptance Testing
- Acceptance Criteria Testing

Exemplo: "Usuário deve conseguir realizar um saque somente após confirmar o 2FA." O teste verifica exatamente esse requisito.

---

## 16. Testes exploratórios

Não seguem necessariamente casos de teste pré-definidos. O tester explora o sistema procurando comportamentos inesperados.

Exemplo:
```
Clicar rápido várias vezes
Voltar durante uma operação
Abrir duas abas
Enviar formulário duplicado
Alterar valores pelo DevTools
Interromper uma requisição
```

Muito úteis para encontrar bugs que testes automatizados não previram.

---

## 17. Testes de usabilidade

- Navegação
- Clareza das informações
- Feedback
- Formulários
- Mensagens de erro
- Fluxos
- UX
- Consistência visual

---

## 18. Testes de acessibilidade

- Teclado
- Screen reader
- Contraste
- Focus
- ARIA
- Labels
- Semântica HTML
- Navegação sem mouse
- Tamanho de elementos
- Mensagens acessíveis

Ferramentas: axe, Lighthouse, WCAG testing

---

## 19. Testes de compatibilidade

### Navegadores
```
Chrome
Firefox
Edge
Safari
```

### Sistemas
```
Windows
Linux
macOS
Android
iOS
```

### Dispositivos
```
Desktop
Notebook
Tablet
Celular
```

---

## 20. Testes de UI

- Componentes
- Botões
- Inputs
- Modais
- Tabelas
- Menus
- Responsividade
- Estados de loading
- Estados vazios
- Estados de erro
- Formulários

---

## 21. Testes visuais — Visual Regression

Comparam screenshots para detectar mudanças visuais inesperadas.

```
Screenshot anterior
        ↓
Screenshot atual
        ↓
Comparação
        ↓
Diferenças
```

Muito útil em frontend.

---

## 22. Testes de responsividade

```
320px
375px
768px
1024px
1440px
1920px
```

---

## 23. Testes de localização / internacionalização

- Português
- Inglês
- Espanhol
- Formatação de moeda
- Datas
- Timezones
- Números
- Pluralização
- Traduções
- RTL

---

## 24. Testes de concorrência

Verificam o comportamento quando várias operações acontecem simultaneamente.

Exemplo:
```
Usuário possui R$100

Request A → sacar R$100
Request B → sacar R$100
```

O sistema precisa impedir que o saldo fique inconsistente.

Muito importante para: Bancos, Exchanges, Jogos, Carteiras, Estoque, Pagamentos

---

## 25. Testes de idempotência

Verificam se repetir a mesma operação não produz efeitos duplicados.

Exemplo:
```
POST /withdraw
Idempotency-Key: ABC123
```

Enviar novamente não deve criar dois saques.

---

## 26. Testes de resiliência

Verificam o comportamento quando alguma dependência falha.

```
Redis caiu
Banco lento
API externa indisponível
RabbitMQ caiu
Blockchain atrasada
Internet caiu
```

Avaliam: Retry, Timeout, Circuit breaker, Fallback, Queue, Recovery

---

## 27. Chaos Testing

É uma forma mais agressiva de testar resiliência. Você **intencionalmente causa falhas**.

```
Matar container
↓
Derrubar serviço
↓
Adicionar latência
↓
Bloquear rede
↓
Verificar recuperação
```

---

## 28. Testes de recuperação

- Backup
- Restore
- Database recovery
- Server recovery
- Crash recovery
- Disaster recovery
- Failover

---

## 29. Testes de disponibilidade

- Health check
- Liveness
- Readiness
- Failover
- Uptime
- Redundância

---

## 30. Testes de arquitetura

Verificam se o código respeita a arquitetura definida.

Exemplo:
```
Controller
   ↓
Service
   ↓
Repository
```

E impedir:
```
Controller
   ↓
Banco diretamente
```

Podem verificar: Dependências, Imports, Camadas, Modularidade, Acoplamento, Circular dependencies, Boundaries

---

## 31. Testes de migração

```
Banco antigo
     ↓
Migration
     ↓
Banco novo
```

Verificam: Dados preservados, Schema correto, Rollback, Compatibilidade, Índices, Constraints

---

## 32. Testes de deploy

- Build
- Docker image
- Environment variables
- Database migration
- Health check
- SSL
- Reverse proxy
- DNS
- CDN
- Workers
- Cron jobs
- PM2/systemd
- Kubernetes

---

## 33. Testes de CI/CD

```
Push
 ↓
Lint
 ↓
Typecheck
 ↓
Unit
 ↓
Integration
 ↓
Build
 ↓
E2E
 ↓
Deploy
```

---

## 34. Testes de configuração

- `.env`
- Secrets
- Database URL
- API keys
- CORS
- Feature flags
- Environment
- Timezone
- URLs
- Ports

---

## 35. Testes de jobs e workers

Para sistemas assíncronos:

- Cron
- Queue
- Worker
- Retry
- Dead-letter queue
- Scheduled jobs
- Duplicate jobs
- Job timeout
- Job failure
- Job recovery

---

## 36. Testes de eventos

Para sistemas event-driven.

```
OrderCreated
PaymentApproved
WithdrawalRequested
MachineActivated
```

Testam: Evento publicado, Payload, Consumidor, Ordem, Duplicação, Retry, Eventual consistency

---

## 37. Testes de cache

- Cache hit
- Cache miss
- Expiração
- Invalidação
- TTL
- Cache stampede
- Dados desatualizados
- Redis failure

---

## 38. Testes de arquivos

- Upload
- Download
- MIME type
- Tamanho máximo
- Arquivo corrompido
- Nome inválido
- Path traversal
- Permissões
- Storage
- CDN

---

## 39. Testes de notificações

- Email
- SMS
- Push
- Web Push
- Discord
- Telegram
- Webhook

Testar:
```
Enviou?
Conteúdo correto?
Destinatário correto?
Não duplicou?
Retry funciona?
```

---

## 40. Testes de blockchain/Web3

Para um projeto como BlockMiner/DEX, existem testes específicos:

- Wallet generation
- Address validation
- Transaction creation
- Transaction signing
- Transaction broadcasting
- Balance tracking
- Confirmation tracking
- Block confirmation
- Nonce management
- Gas/fee calculation
- RPC failure
- RPC timeout
- Chain reorganization
- Double-spend protection
- Deposit detection
- Withdrawal processing
- Blockchain reconciliation
- Hot wallet
- Cold wallet
- Signature validation
- Transaction idempotency

---

## 41. Testes financeiros

Extremamente importantes em sistemas de dinheiro.

- Saldo
- Crédito
- Débito
- Transferência
- Depósito
- Saque
- Taxa
- Spread
- Conversão
- Arredondamento
- Decimal precision
- Double-spending
- Race condition
- Reembolso
- Estorno
- Ledger
- Reconciliação
- Idempotência

---

## 42. Testes de dados

- Data integrity
- Data validation
- Data consistency
- Duplicate detection
- Null handling
- Encoding
- Serialization
- Deserialization
- Importação
- Exportação

---

## 43. Testes de observabilidade

- Logs
- Metrics
- Traces
- Health checks
- Alerts
- Error tracking
- Audit logs
- Correlation IDs

---

## 44. Testes de logging

- Log correto
- Nível correto
- Contexto
- Correlation ID
- Não vazar senha
- Não vazar token
- Não vazar API key
- Não vazar dados sensíveis

---

## 45. Testes de auditoria

Muito importantes em sistemas financeiros/admin. Verificam se operações importantes deixam rastreabilidade:

```
Quem?
O quê?
Quando?
De onde?
Qual recurso?
Qual resultado?
```

---

## 46. Testes de permissões

Por exemplo:
```
USER
MODERATOR
ADMIN
SUPER_ADMIN
```

Testar: Pode acessar? Não pode acessar? Pode editar? Pode deletar? Pode visualizar? Pode executar determinada ação?

---

## 47. Testes de configuração de produção

Antes de liberar:

- Production env
- Database
- SSL
- DNS
- Secrets
- CORS
- Firewall
- Rate limits
- Storage
- Queue
- Cron
- Monitoring
- Backup

---

## 48. Testes de instalação

- Fresh install
- Upgrade
- Downgrade
- Migration
- Dependencies
- Environment setup

---

## 49. Testes de atualização

```
v1.0
 ↓
v1.1
 ↓
v1.2
```

Verificar: Dados, Configuração, Compatibilidade, Migration, APIs, Frontend, Jobs

---

## 50. Testes de rollback

Se o deploy quebrar:

```
v2.0
 ↓
❌ problema
 ↓
rollback
 ↓
v1.9
```

O sistema deve voltar ao estado funcional.

---

## 51. Testes de mutação — Mutation Testing

Modifica propositalmente o código para verificar se seus testes conseguem detectar o erro.

Exemplo:
```ts
if (balance >= amount)
```
vira:
```ts
if (balance > amount)
```

Se todos os testes continuam passando, sua suíte provavelmente não está cobrindo aquele comportamento adequadamente.

---

## 52. Testes baseados em propriedades — Property-Based Testing

Em vez de testar apenas exemplos específicos, testa **propriedades que sempre devem ser verdadeiras**.

Exemplo:
```
saldo nunca pode ficar negativo
```

Testa milhares de combinações automaticamente.

---

## 53. Fuzz Testing

Envia entradas inesperadas, aleatórias ou malformadas.

```
strings gigantes
números extremos
JSON inválido
Unicode
payloads aleatórios
```

Objetivo: encontrar crashes, bugs e vulnerabilidades.

---

## 54. Testes de contrato de dados

Especialmente para: JSON, OpenAPI, GraphQL, eventos, mensagens de fila.

Verificam:
```
Campo existe?
Tipo correto?
Obrigatório?
Formato correto?
```

---

## 55. Testes de GraphQL

- Query
- Mutation
- Subscription
- Schema
- Resolver
- Authorization
- N+1
- Pagination
- Query depth
- Query complexity
- Introspection

---

## 56. Testes de WebSocket

- Connection
- Authentication
- Reconnection
- Disconnect
- Message
- Broadcast
- Room
- Ordering
- Duplicate messages
- Connection timeout

---

## 57. Testes de microserviços

```
Auth
 ↓
Users
 ↓
Payments
 ↓
Notifications
```

Testar: Comunicação, Contratos, Retry, Timeout, Circuit breaker, Service discovery, Eventual consistency, Distributed transactions, Tracing

---

## 58. Testes de carga de banco

- SELECT
- INSERT
- UPDATE
- DELETE
- JOIN
- Index
- Lock
- Connection pool
- Concurrent transactions

---

## 59. Testes de infraestrutura

- Docker
- Kubernetes
- Nginx
- Load balancer
- DNS
- Firewall
- Storage
- Volumes
- Network
- TLS
- Certificates

---

## 60. Testes de container

- Build
- Startup
- Healthcheck
- Environment
- Volumes
- Networking
- Resource limits
- Graceful shutdown
- Restart

---

## 61. Testes de Kubernetes

- Pod
- Deployment
- Service
- Ingress
- ConfigMap
- Secret
- HPA
- Readiness
- Liveness
- Rolling update
- Rollback
- Resource limits

---

## 62. Testes de documentação

- OpenAPI
- README
- API examples
- Installation instructions
- Environment variables
- Architecture documentation

O objetivo é verificar se a documentação corresponde ao comportamento real.

---

## 63. Testes de código estático

Não executam necessariamente a aplicação.

- ESLint
- TypeScript
- SonarQube
- Semgrep
- Dependency scanning
- Dead code detection
- Complexity analysis
- Formatting

---

## 64. Testes de build

```
Install
 ↓
Typecheck
 ↓
Compile
 ↓
Bundle
 ↓
Build
```

---

## 65. Testes de dependências

- Vulnerabilidades
- Licenças
- Versões
- Dependências quebradas
- Dependências transitivas
- Lockfile

---

## 66. Testes de compatibilidade retroativa

Verificam se uma nova versão continua funcionando com clientes antigos. Muito importante para APIs:

```
API v1
 ↓
API v2
```

O cliente antigo ainda deve funcionar quando isso for requisito.

---

## 67. Testes de migração de API

```
v1 → v2
```

Verificam: Endpoints, Payloads, Auth, Headers, Responses, Deprecation, Backward compatibility

---

## 68. Testes de aceitação de negócio

Além de "funciona?", verifica: **"Funciona de acordo com a regra comercial?"**

Exemplo:
```
Máquina custa R$100
Taxa = 5%
Saldo final = R$95
```

---

## 69. Testes de regras de negócio

Um dos mais importantes para seus projetos.

Exemplos:
```
Usuário não pode sacar sem saldo.
Usuário precisa de 2FA.
Máquina congelada não minera.
Recompensa não pode ser creditada duas vezes.
```

---

## 70. Testes de segurança operacional

- Secret rotation
- Key rotation
- Token expiration
- Session invalidation
- Account lockout
- Audit trail
- Admin actions
- Recovery procedures

---

## Como organizar isso em um projeto real

Não é necessário colocar todos os 70 tipos em todos os projetos.

Para um projeto Web moderno, uma pirâmide aproximada seria:

```
                    E2E
                   /   \
              System / Acceptance
                /         \
          Integration     API
             /              \
        Unit Tests      Contract Tests
```

E por fora:

```
Security
Performance
Accessibility
Visual Regression
Resilience
Infrastructure
Database
Observability
```

### Pipeline ideal

```
┌──────────────────────────┐
│       Developer          │
└────────────┬─────────────┘
             ↓
        Lint / Format
             ↓
         Typecheck
             ↓
       Unit Tests
             ↓
   Integration Tests
             ↓
       API Tests
             ↓
     Contract Tests
             ↓
        Build Test
             ↓
       E2E / System
             ↓
    Security Scanning
             ↓
     Performance Tests
             ↓
          Deploy
             ↓
       Smoke Tests
             ↓
       Production
```

E depois do deploy:

```
Production
    ↓
Health Checks
    ↓
Monitoring
    ↓
Logs
    ↓
Metrics
    ↓
Alerts
```

**Recomendação prática:** separar os testes em `unit`, `integration`, `api`, `e2e`, `security`, `performance`, `contract`, `smoke` e `regression`, em vez de criar dezenas de categorias de pastas. Isso mantém a estrutura modular sem transformar o projeto numa burocracia de testes.

# Checklist de Segurança para Projetos de Software

Catálogo genérico (OWASP/CWE) — **não** é o contrato do SatsPay. O contrato está em [`docs/security/threat-model-and-gaps.md`](../security/threat-model-and-gaps.md) e no addendum [`docs/security/audit-2026-09-01.md`](../security/audit-2026-09-01.md). Não implementar os ~200 itens; usar só classes que batem (auth, IDOR, injection via `bind`, secrets, rate-limit).

Lista ampla de vulnerabilidades organizadas por categoria, baseada principalmente nas classes de problemas normalmente tratadas em OWASP, CWE e segurança de aplicações.

## 🔐 1. Autenticação

Problemas relacionados a provar quem é o usuário.

- Senha fraca
- Política de senha inadequada
- Brute force
- Credential stuffing
- Password spraying
- Ausência de rate limiting
- Ausência de account lockout
- Login enumeration
- User enumeration
- Timing attack
- Recuperação de senha insegura
- Token de recuperação previsível
- Token de recuperação reutilizável
- Token de recuperação sem expiração
- MFA/2FA bypass
- OTP reutilizável
- OTP sem expiração
- OTP previsível
- Magic link reutilizável
- Session fixation
- Session hijacking
- Credenciais expostas
- Default credentials
- Autenticação incompleta
- Autenticação bypass
- JWT mal configurado
- OAuth misconfiguration
- OpenID Connect misconfiguration

---

## 👤 2. Autorização / Controle de acesso

Uma das categorias **mais importantes** em aplicações Web.

- IDOR
- BOLA — Broken Object Level Authorization
- BFLA — Broken Function Level Authorization
- Privilege escalation
- Vertical privilege escalation
- Horizontal privilege escalation
- Admin access bypass
- Role bypass
- Permission bypass
- Tenant isolation failure
- Missing authorization
- Insecure direct object reference
- Manipulação de `user_id`
- Manipulação de `account_id`
- Manipulação de `wallet_id`
- Acesso a recursos de outro usuário
- Acesso indevido a endpoints administrativos

Exemplo clássico:

```
GET /api/users/123/profile
```

Usuário 123 troca para:

```
GET /api/users/124/profile
```

e consegue acessar os dados do usuário 124.

---

## 💉 3. Injection

Entrada controlada pelo usuário sendo interpretada como código/comando.

- SQL Injection
- NoSQL Injection
- LDAP Injection
- XPath Injection
- XML Injection
- OS Command Injection
- Shell Injection
- Template Injection
- SSTI
- Expression Language Injection
- GraphQL Injection
- Header Injection
- CRLF Injection
- CSV Injection
- ORM Injection

---

## 🌐 4. XSS — Cross-Site Scripting

Execução de JavaScript no navegador da vítima.

### Stored XSS
Payload fica armazenado.
```
Banco
 ↓
Aplicação
 ↓
Outro usuário
```

### Reflected XSS
Payload vem da requisição e volta na resposta.

### DOM XSS
A vulnerabilidade acontece no JavaScript do navegador.

Também:

- Mutation XSS
- Self-XSS
- XSS em atributos
- XSS em templates
- XSS via Markdown
- XSS via SVG
- XSS via upload

---

## 🔄 5. CSRF

Força um usuário autenticado a realizar uma ação que não pretendia.

Exemplo:
```
Usuário logado
      ↓
Site malicioso
      ↓
POST /api/change-email
      ↓
Alteração indevida
```

Proteções:

- CSRF token
- SameSite cookies
- Origin validation
- Referer validation

---

## 📨 6. SSRF

**Server-Side Request Forgery.**

O atacante faz o servidor realizar requisições para destinos que ele normalmente não deveria acessar.

Possíveis alvos:
```
localhost
127.0.0.1
rede interna
metadata service
serviços internos
```

Muito relevante em: APIs, Cloud, Microserviços, Webhooks, URL fetchers, Image importers

---

## 📁 7. Path Traversal

Permite acessar arquivos fora do diretório permitido.

Exemplo conceitual:
```
/download?file=../../arquivo
```

Variantes:

- Directory traversal
- File traversal
- Null byte
- Encoded traversal
- Double encoding

---

## 📂 8. File Upload

Upload inseguro pode permitir:

- Upload de arquivo executável
- Web shell
- XSS via arquivo
- SVG XSS
- Malware upload
- Path traversal
- MIME spoofing
- Extension spoofing
- Double extension
- ZIP bomb
- Arquivo gigante
- Storage abuse
- Overwrite de arquivos

---

## 🖥️ 9. Command Injection

Entrada do usuário acaba executada pelo sistema operacional.

Exemplo conceitual:
```
usuário → aplicação → shell
```

Pode resultar em: execução arbitrária, acesso a arquivos, alteração do servidor, comprometimento da aplicação

---

## 🗄️ 10. Banco de dados

- SQL Injection
- NoSQL Injection
- Excessive database privileges
- Database exposed to internet
- Default credentials
- Weak database password
- Unencrypted database connection
- Sensitive data exposure
- Missing constraints
- Race conditions
- Data corruption
- Unauthorized queries
- Excessive database permissions
- Backup exposure
- Database dump exposure

---

## 🔑 11. Secrets / Credenciais

- API keys no código
- Senhas no Git
- Tokens no Git
- Private keys expostas
- `.env` exposto
- JWT secrets fracos
- Database credentials expostas
- Cloud credentials
- SSH keys
- Hardcoded credentials
- Secrets em logs
- Secrets em frontend
- Secrets em Docker image
- Secrets em CI/CD
- Secrets em backups

---

## 🍪 12. Sessão / Cookies

- Session fixation
- Session hijacking
- Session prediction
- Session replay
- Session timeout inexistente
- Logout incompleto
- Cookie sem `HttpOnly`
- Cookie sem `Secure`
- Cookie sem `SameSite`
- Session token na URL
- Token armazenado de maneira insegura
- Sessões simultâneas não controladas

---

## 🎫 13. JWT

Problemas comuns:

- JWT secret fraco
- Token sem expiração
- `alg: none`
- Algorithm confusion
- HS/RS confusion
- Aceitar token expirado
- Não validar issuer
- Não validar audience
- Não validar signature
- JWT armazenado inseguramente
- Token muito longo
- Refresh token sem rotação
- Refresh token reutilizável
- Revogação inexistente

---

## 🌍 14. CORS

Configuração incorreta pode permitir acesso indevido.

- `Access-Control-Allow-Origin: *`
- Origin reflection
- Credentials + wildcard
- Origem não validada
- Métodos excessivos
- Headers excessivos
- CORS em endpoints sensíveis

---

## 🧱 15. Security Misconfiguration

Uma categoria gigantesca.

- Debug em produção
- Stack trace exposto
- Admin panel exposto
- Swagger exposto sem proteção
- Portas desnecessárias abertas
- Serviços desnecessários
- Default configuration
- Default credentials
- Directory listing
- `.git` público
- `.env` público
- Backup público
- Logs públicos
- Headers de segurança ausentes
- TLS mal configurado
- CORS incorreto
- Permissões de arquivos incorretas

---

## 📡 16. HTTP

- HTTP request smuggling
- HTTP response splitting
- CRLF injection
- Host header injection
- HTTP parameter pollution
- Request desynchronization
- Cache poisoning
- Cache deception
- Insecure redirects

---

## 🔀 17. Open Redirect

Aplicação permite redirecionamento para URL controlada pelo atacante.

Exemplo:
```
/login?redirect=https://site-malicioso.com
```

Pode ser usado em: phishing, OAuth attacks, token leakage, social engineering

---

## 🧠 18. Business Logic

Aqui estão bugs que muitas ferramentas automáticas não encontram.

- Manipulação de preço
- Manipulação de saldo
- Cupom reutilizável
- Recompensa duplicada
- Cashback duplicado
- Bypass de limite
- Bypass de etapa
- Operação fora de ordem
- Race condition
- Double spending
- Negative quantity
- Negative balance
- Negative price
- Tax bypass
- Fee bypass
- Referral abuse
- Reward abuse
- Withdrawal bypass
- Deposit duplication
- Replay de operação

Para sistemas financeiros, isso é **crítico**.

---

## 🏦 19. Vulnerabilidades financeiras

Especialmente importantes em fintech, exchanges, wallets e sistemas de pagamentos.

- Double spending
- Saldo negativo
- Crédito duplicado
- Débito duplicado
- Saque duplicado
- Depósito duplicado
- Race condition
- Floating-point error
- Decimal precision error
- Rounding error
- Fee manipulation
- Price manipulation
- Exchange-rate manipulation
- Replay attack
- Transaction duplication
- Idempotency failure
- Ledger inconsistency
- Balance desynchronization
- Reconciliation failure

---

## ⚡ 20. Race Conditions

Duas operações acontecem simultaneamente e quebram uma regra.

Exemplo:
```
Saldo = R$100

Request A → sacar R$100
Request B → sacar R$100

Resultado vulnerável:
R$100 - R$100 - R$100
```

Isso pode aparecer em: saque, compra, estoque, recompensas, cupons, créditos, transferências, máquinas, mineração, pagamentos

---

## 🔁 21. Replay Attacks

Uma operação válida é capturada e repetida.

Exemplo:
```
Transaction A
     ↓
replay
     ↓
Transaction A
     ↓
Transaction A
```

Proteções: nonce, timestamp, expiration, idempotency key, unique transaction ID, replay protection

---

## 🧮 22. Integer / Numeric Vulnerabilities

- Integer overflow
- Integer underflow
- Signed/unsigned confusion
- Negative numbers
- Decimal precision
- Floating-point precision
- Rounding
- Scientific notation abuse
- Extremely large numbers

---

## 🧬 23. Deserialization

- Insecure deserialization
- Object injection
- Prototype pollution
- Unsafe serialization
- Unsafe YAML
- Unsafe pickle
- Unsafe object reconstruction

---

## 🟨 24. JavaScript / Node.js

Algumas categorias especialmente relevantes:

- Prototype Pollution
- ReDoS
- Command Injection
- Path Traversal
- Package vulnerabilities
- Dependency confusion
- Malicious npm package
- `eval()`
- `Function()`
- Unsafe child processes
- Unsafe dynamic imports
- SSRF
- Unsafe template rendering

---

## 🐍 25. Python

- Pickle deserialization
- `eval`
- `exec`
- Command injection
- SSTI
- YAML deserialization
- Path traversal
- Dependency vulnerabilities

---

## 🦀 26. Rust

Rust elimina várias classes tradicionais de memory-safety bugs, mas **não elimina vulnerabilidades de aplicação**.

Ainda podem existir:

- SQL Injection
- SSRF
- XSS
- CSRF
- IDOR/BOLA
- Authorization bypass
- Command injection
- Path traversal
- Race conditions
- Logic bugs
- Integer overflow
- Unsafe code vulnerabilities
- Dependency vulnerabilities
- Supply-chain attacks
- Cryptographic misuse

E especificamente:

- `unsafe` misuse
- FFI vulnerabilities
- panics causando DoS
- `unwrap()` em caminhos controlados
- concorrência incorreta
- problemas em crates/dependências

---

## 📦 27. Dependências / Supply Chain

- Vulnerable dependency
- Malicious dependency
- Dependency confusion
- Typosquatting
- Compromised package
- Malicious update
- Transitive dependency vulnerability
- Abandoned dependency
- Dependency hijacking
- Lockfile manipulation
- Build dependency compromise

---

## 🐳 28. Docker / Containers

- Container running as root
- Privileged container
- Exposed Docker socket
- Secrets dentro da image
- Vulnerable base image
- Excessive capabilities
- Host filesystem mounted
- Docker daemon exposure
- Insecure registry
- Untrusted image
- Container escape
- Missing resource limits

---

## ☸️ 29. Kubernetes

- Excessive RBAC
- Privileged pods
- HostPath abuse
- HostNetwork
- HostPID
- Exposed API server
- Weak service accounts
- Secrets exposure
- Insecure ingress
- Network policies ausentes
- Container running as root
- Missing resource limits
- Misconfigured admission controls

---

## ☁️ 30. Cloud

AWS/Azure/GCP etc.

- Public storage bucket
- Exposed credentials
- Excessive IAM permissions
- Public database
- Public server
- Security group misconfiguration
- Metadata service abuse
- SSRF → cloud credentials
- Secret exposure
- Misconfigured KMS
- Insecure storage
- Weak IAM policies

---

## 🌐 31. API Security

- BOLA
- BFLA
- Broken authentication
- Broken authorization
- Excessive data exposure
- Mass assignment
- Unrestricted resource consumption
- Rate-limit bypass
- API enumeration
- Improper inventory management
- Shadow APIs
- Deprecated APIs
- Excessive permissions
- Missing input validation

---

## 📊 32. Mass Assignment

O cliente envia campos que não deveria poder modificar.

Exemplo:
```json
{
  "name": "Gustavo",
  "role": "admin"
}
```

Se o backend simplesmente fizer:
```
user.update(body)
```

pode existir privilege escalation.

---

## 📤 33. Excessive Data Exposure

API retorna mais dados do que o frontend realmente precisa.

Por exemplo:
```json
{
  "id": 10,
  "name": "...",
  "email": "...",
  "passwordHash": "...",
  "internalNotes": "...",
  "adminFlags": "..."
}
```

Mesmo que o frontend não mostre esses campos, eles foram expostos.

---

## 🚦 34. Rate Limiting / DoS

- Brute force
- API flooding
- Resource exhaustion
- Memory exhaustion
- CPU exhaustion
- Connection exhaustion
- Database connection exhaustion
- Queue flooding
- Regex DoS
- ZIP bomb
- Large payload
- Large file upload

---

## 💥 35. Denial of Service

Pode ocorrer por:

- CPU exhaustion
- Memory exhaustion
- Disk exhaustion
- Connection exhaustion
- Thread exhaustion
- Database exhaustion
- Queue exhaustion
- Infinite loop
- ReDoS
- Crash/panic
- Malformed input

---

## 🧵 36. ReDoS

**Regular Expression Denial of Service.**

Regex mal construída pode consumir CPU excessivamente com determinada entrada.

---

## 📧 37. Email / Mensageria

- Email spoofing
- Header injection
- SMTP abuse
- Email enumeration
- Spam abuse
- Account takeover via email
- Password-reset abuse
- Verification-token reuse

---

## 🔔 38. Webhooks

- Webhook spoofing
- Signature bypass
- Replay
- Missing authentication
- SSRF
- Duplicate events
- Event injection
- Payload tampering
- Missing idempotency
- Timestamp validation ausente

---

## 🔗 39. OAuth

- Redirect URI manipulation
- State parameter missing
- CSRF
- Authorization code leakage
- Token leakage
- PKCE missing
- Open redirect
- Account linking flaw
- Account takeover

---

## 🪪 40. SSO / Identity

- SAML misconfiguration
- Assertion manipulation
- Signature validation failure
- Audience validation failure
- Issuer validation failure
- Replay
- Account mapping flaw
- Identity spoofing

---

## 📱 41. Mobile

- Insecure local storage
- Hardcoded secrets
- Certificate pinning issues
- Insecure WebViews
- Deep-link abuse
- Intent abuse
- Root detection bypass
- Debug builds
- Excessive permissions
- API key exposure

---

## 🖥️ 42. Frontend

- XSS
- DOM XSS
- Sensitive data in localStorage
- Token exposure
- Source map exposure
- API keys expostas
- Client-side authorization
- Hidden admin routes
- Sensitive information in JS bundle
- Prototype pollution
- Unsafe HTML rendering

**Importante:** esconder botão no frontend **não é autorização**.

---

## 📱 43. Local Storage / Browser Storage

- JWT no localStorage
- Refresh token no localStorage
- Dados sensíveis armazenados
- Cache de informações privadas
- IndexedDB exposure
- Session data exposure

---

## 🧾 44. Logs

- Password logging
- Token logging
- API key logging
- Personal data logging
- Wallet private key logging
- Sensitive payload logging
- Log injection
- Log forging
- Excessive logging
- Logs acessíveis publicamente

---

## 🔍 45. Information Disclosure

- Stack trace
- Source code
- `.git`
- `.env`
- Backup
- Database dump
- Internal IP
- Server version
- Framework version
- Dependency version
- Debug endpoints
- API documentation
- Internal error messages

---

## 🗂️ 46. Backup

- Backup público
- Backup sem criptografia
- Backup com credenciais
- Backup acessível via HTTP
- Backup antigo não removido
- Banco exposto
- Snapshot público
- Storage bucket público

---

## 🔐 47. Criptografia

- Weak encryption
- Weak hash
- MD5
- SHA-1 para senhas
- Weak random number generator
- Hardcoded encryption key
- Reused IV/nonce
- ECB mode
- Improper key management
- Plaintext secrets
- TLS misconfiguration
- Certificate validation failure

Para senhas:

> **Nunca usar criptografia reversível para armazenar senha.**

Use hashing adaptativo como Argon2id, bcrypt ou scrypt.

---

## 🎲 48. Randomness

- Predictable tokens
- Predictable OTP
- Predictable reset links
- Weak session IDs
- Weak nonce
- Weak salts
- `Math.random()` para segurança
- Insufficient entropy

---

## ⛓️ 49. Blockchain / Web3

Para DEX, wallet ou sistema como BlockMiner:

- Private key exposure
- Seed phrase exposure
- Hot-wallet compromise
- Reused nonce
- Transaction replay
- Double spending
- Fake deposit
- Deposit spoofing
- Confirmation bypass
- Reorg handling failure
- RPC manipulation
- RPC trust issues
- Chain ID validation failure
- Signature validation failure
- Incorrect transaction verification
- Token contract spoofing
- Decimal mismatch
- Balance desynchronization
- Fee manipulation
- Price oracle manipulation
- Oracle failure
- MEV-related issues
- Front-running
- Sandwich attacks
- Slippage manipulation
- Approval abuse
- Infinite token allowance
- Smart-contract vulnerabilities

---

## 📜 50. Smart Contracts

Uma lista própria:

- Reentrancy
- Access control
- Integer overflow/underflow
- Oracle manipulation
- Flash-loan attacks
- Price manipulation
- Front-running
- Back-running
- Sandwich
- Signature replay
- Missing nonce
- Delegatecall abuse
- `tx.origin` authentication
- Unchecked external calls
- Denial of service
- Gas griefing
- Storage collision
- Proxy misconfiguration
- Upgradeability abuse
- Initialization attack
- Selfdestruct-related issues
- Precision loss
- Rounding
- Incorrect accounting

---

## 🤖 51. IA / LLM

Em aplicações com IA:

- Prompt Injection
- Indirect Prompt Injection
- Jailbreak
- System prompt leakage
- Sensitive data disclosure
- Excessive agency
- Tool abuse
- Insecure tool permissions
- Data poisoning
- Model poisoning
- RAG poisoning
- Vector database access control failure
- Embedding leakage
- Insecure output handling
- SSRF via tools
- Command execution via agent
- Unauthorized actions
- Excessive permissions

---

## 🧠 52. Agent / AI Coding Tools

Quando um agente tem acesso ao sistema:

- Arbitrary file modification
- Command execution
- Secret exposure
- Prompt injection through repository files
- Malicious dependency installation
- Supply-chain compromise
- Data exfiltration
- Unauthorized network access
- Privilege escalation
- Destructive commands
- Tool abuse

Isso é particularmente importante quando se usa um agente com **permissões amplas no terminal**.

---

## 🧪 53. Vulnerabilidades de teste

Até a infraestrutura de testes pode gerar problemas:

- Credenciais reais em testes
- Banco de produção usado em testes
- API keys reais
- Dados pessoais reais
- Test endpoints expostos
- Debug endpoints
- Mock bypass
- Test accounts privilegiadas
- Backdoors temporários esquecidos

---

## 🏗️ 54. Arquitetura

- Excessive trust
- Single point of failure
- Missing isolation
- Excessive coupling
- Insecure trust boundaries
- Shared database abuse
- Missing tenant isolation
- Privilege concentration
- Lack of defense in depth

---

## 🏢 55. Multi-tenancy

Para SaaS:
```
Empresa A
   ↓
tenant_id = A

Empresa B
   ↓
tenant_id = B
```

Vulnerabilidades:

- Tenant IDOR
- Missing tenant filter
- Cross-tenant data leakage
- Cross-tenant modification
- Cross-tenant deletion
- Shared cache leakage
- Shared storage leakage
- Incorrect authorization

---

## 🗑️ 56. Deleção de dados

- Unauthorized deletion
- Soft-delete bypass
- Deleted records accessible
- Deleted records restored improperly
- Cascade deletion abuse
- IDOR via deleted resources
- Physical deletion de dados que deveriam ser auditáveis

---

## 🧹 57. Sanitização / Validação

- Missing validation
- Client-side-only validation
- Type confusion
- Null handling
- Boundary bypass
- Unicode normalization
- Encoding confusion
- Canonicalization bugs
- Unexpected types
- Unexpected arrays
- Duplicate parameters

---

## 🧩 58. Encoding

- Double encoding
- URL encoding bypass
- Unicode normalization
- UTF-8 issues
- Null bytes
- HTML encoding bypass
- Path canonicalization
- Case normalization

---

## 🕵️ 59. Privacy

- Excessive data collection
- Data leakage
- Incorrect access
- Sensitive data exposure
- PII in logs
- PII in analytics
- PII in URLs
- PII in error messages
- Missing data deletion
- Missing retention policy

---

## 📋 60. Auditoria

- Missing audit logs
- Audit log manipulation
- Missing actor information
- Missing timestamp
- Missing IP/context
- Admin action not recorded
- Financial operation not recorded
- Logs deletable by same administrator being audited

---

## 🛡️ Checklist prático

Para um projeto Web/Node/Rust/API, considerar pelo menos:

```
SECURITY
│
├── Authentication
├── Authorization
├── Session
├── JWT
├── OAuth
├── CSRF
├── CORS
│
├── Input
│   ├── SQL Injection
│   ├── XSS
│   ├── SSRF
│   ├── Command Injection
│   ├── Path Traversal
│   └── SSTI
│
├── API
│   ├── IDOR/BOLA
│   ├── BFLA
│   ├── Mass Assignment
│   ├── Rate Limit
│   └── Data Exposure
│
├── Business Logic
│   ├── Race Conditions
│   ├── Replay
│   ├── Double Spending
│   ├── Price Manipulation
│   ├── Balance Manipulation
│   └── Reward Abuse
│
├── Database
│   ├── Injection
│   ├── Permissions
│   ├── Transactions
│   ├── Backups
│   └── Data Integrity
│
├── Secrets
│   ├── API Keys
│   ├── JWT Secrets
│   ├── Database Passwords
│   └── Private Keys
│
├── Infrastructure
│   ├── Docker
│   ├── Kubernetes
│   ├── Nginx
│   ├── TLS
│   └── Cloud
│
├── Dependencies
│   ├── CVEs
│   ├── Supply Chain
│   ├── Dependency Confusion
│   └── Malicious Packages
│
├── Web3
│   ├── Wallet
│   ├── Blockchain
│   ├── RPC
│   ├── Replay
│   ├── Oracle
│   └── Smart Contract
│
└── Monitoring
    ├── Logs
    ├── Audit
    ├── Alerts
    └── Incident Response
```

### E uma coisa importante

**Vulnerabilidade não é sinônimo de CVE.**

Uma vulnerabilidade pode ser:

- **CVE** → vulnerabilidade identificada em um produto/componente.
- **CWE** → categoria/tipo de fraqueza no software.
- **OWASP** → classificação/práticas de segurança de aplicações.
- **Misconfiguration** → configuração insegura.
- **Business Logic Flaw** → falha na regra do negócio.
- **Design Flaw** → problema arquitetural/de projeto.
- **Implementation Bug** → erro de implementação.

E uma aplicação pode estar **100% livre de CVEs conhecidas e ainda ser extremamente vulnerável** por causa de IDOR, autorização, race condition, lógica financeira, exposição de dados etc.

Para projetos como **BlockMiner/DEX/wallet**, a prioridade máxima deve ser dada a **autorização, lógica financeira, concorrência, idempotência, replay, integridade de saldo/ledger, autenticação, secrets, blockchain/RPC e auditoria**, além das vulnerabilidades Web tradicionais.

# Domain SAST (CodeQL + custom Semgrep)

## Custom Semgrep (BitcoSats)

Rules: [`.semgrep/bitcosats.yml`](../../.semgrep/bitcosats.yml)

```bash
scripts/run_security_sast.sh
# or
semgrep scan --config .semgrep/bitcosats.yml .
```

Catches regressions / debt that generic packs miss:

| Rule | Intent |
|------|--------|
| hardcoded webhook HMAC | no shared `satspay_secret_default` |
| simulate-payment | public credit-without-payer must stay gone |
| confirm_invoice in api-http | no HTTP ledger credit |
| CF-Connecting-IP trust | spoofable client IP |
| fee-margin fail-open | regression on hard-block |
| admin approve OTP conditional | regression: approve must not gate OTP on smtp/2FA only |
| `e.to_string()` in JSON errors | info leak (WARNING); **5xx + `e.to_string()` é ERROR** — CI falha |
| empty AAD / `.encrypt(` | AES-GCM binding |
| `useA() \|\| useB()` | React #311 |

Artifacts (gitignored): `.security-audit/semgrep-bitcosats.{json,txt}`

CI: [`.github/workflows/semgrep-bitcosats.yml`](../../.github/workflows/semgrep-bitcosats.yml) **falha o PR** se houver finding ERROR. `cargo deny check` roda em [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) (`deny.toml`).

## CodeQL

- Workflow: [`.github/workflows/codeql.yml`](../../.github/workflows/codeql.yml) — `javascript-typescript` + `rust`, `security-extended`
- Custom JS queries: [`.github/codeql/javascript/`](../../.github/codeql/javascript/) (hooks short-circuit, admin approve emailCode)
- Local (optional): install CLI under `~/tools/codeql`, then `scripts/run_security_sast.sh`

GitHub Code Scanning uploads SARIF on push/PR when the repo has Advanced Security / code scanning enabled.

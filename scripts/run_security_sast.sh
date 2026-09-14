#!/usr/bin/env bash
# Domain SAST for BitcoSats: custom Semgrep + optional CodeQL (JS/TS).
# Docs: docs/security/SAST.md
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
OUT="${SECURITY_AUDIT_OUT:-$ROOT/.security-audit}"
mkdir -p "$OUT"
export PATH="${HOME}/.local/bin:${HOME}/tools/codeql:${PATH}"

echo "==> Semgrep (BitcoSats custom rules)"
semgrep scan \
  --config "$ROOT/.semgrep/bitcosats.yml" \
  --exclude='target' --exclude='node_modules' --exclude='.git' \
  --exclude='.security-audit' --exclude='client/coverage*' \
  --json -o "$OUT/semgrep-bitcosats.json" \
  .
semgrep scan \
  --config "$ROOT/.semgrep/bitcosats.yml" \
  --exclude='target' --exclude='node_modules' --exclude='.git' \
  --exclude='.security-audit' --exclude='client/coverage*' \
  --text -o "$OUT/semgrep-bitcosats.txt" \
  . >/dev/null || true

python3 - <<PY
import json
from pathlib import Path
out = Path("$OUT")
sg = json.loads((out / "semgrep-bitcosats.json").read_text())
rows = sg.get("results") or []
print(f"\\n=== BitcoSats Semgrep: {len(rows)} finding(s) ===")
by = {}
for r in rows:
    sev = (r.get("extra") or {}).get("severity", "?")
    by[sev] = by.get(sev, 0) + 1
errs = [r for r in rows if (r.get("extra") or {}).get("severity") == "ERROR"]
for r in errs:
    print(f"  [ERROR] {r['path']}:{r['start']['line']}  {r.get('check_id')}")
print("by severity:", by)
PY

if command -v codeql >/dev/null 2>&1; then
  echo "==> CodeQL (javascript-typescript on client/)"
  DB="$OUT/codeql-db-js"
  if [[ ! -d "$DB" ]] || [[ "${CODEQL_RECREATE:-}" == "1" ]]; then
    rm -rf "$DB"
    codeql database create "$DB" \
      --language=javascript-typescript \
      --source-root="$ROOT/client" \
      --overwrite 2>"$OUT/codeql-create.err"
  fi
  codeql database analyze "$DB" \
    --format=sarif-latest \
    --output="$OUT/codeql-js.sarif" \
    codeql/javascript-queries:codeql-suites/javascript-security-extended.qls \
    2>"$OUT/codeql-analyze.err" || true
  if [[ -f "$ROOT/.github/codeql/javascript/qlpack.yml" ]]; then
    codeql pack install "$ROOT/.github/codeql/javascript" >/dev/null
    codeql database analyze "$DB" \
      --format=sarif-latest \
      --output="$OUT/codeql-js-custom.sarif" \
      "$ROOT/.github/codeql/javascript" \
      2>>"$OUT/codeql-analyze.err" || true
  fi
  echo "CodeQL SARIF → $OUT/codeql-js.sarif (+ codeql-js-custom.sarif)"
else
  echo "==> CodeQL CLI not on PATH (optional: ~/tools/codeql) — CI workflow still added"
fi

echo "Artifacts under $OUT"

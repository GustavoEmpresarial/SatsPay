#!/usr/bin/env bash
# Batch test runner — unit (Rust + client) then optional smoke suites.
#
# Usage:
#   scripts/run_tests.sh              # unit only (fast)
#   scripts/run_tests.sh unit         # same
#   scripts/run_tests.sh --smoke      # unit + client docker smoke
#   scripts/run_tests.sh --batch      # alias for --smoke (lote)
#   scripts/run_tests.sh --all        # unit + smoke + rust examples (needs DATABASE_URL)

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MODE="${1:-unit}"
case "$MODE" in
  unit|--unit|"") MODE=unit ;;
  --smoke|--batch|smoke|batch) MODE=smoke ;;
  --all|all) MODE=all ;;
  *) echo "Unknown mode: $MODE (use unit|smoke|batch|all)"; exit 2 ;;
esac

FAIL=0

echo "=== [lote] mode=$MODE ==="
echo "=== [1] cargo test --workspace (unit) ==="
if cargo test --workspace --lib --bins --quiet; then
  RUST_N=$(cargo test --workspace --lib --bins -- --list 2>/dev/null | grep -c ': test$' || echo 0)
  echo "[✔] rust unit ok ($RUST_N listed)"
else
  FAIL=1
  echo "[X] rust unit failed"
fi

echo "=== [2] client vitest unit ==="
if (cd client && npx vitest run tests/unit --reporter=dot); then
  echo "[✔] client unit ok"
else
  FAIL=1
  echo "[X] client unit failed"
fi

if [[ "$MODE" == "smoke" || "$MODE" == "all" ]]; then
  echo "=== [3] client vitest smoke (docker postgres) ==="
  if (cd client && npx vitest run tests/smoke --reporter=dot); then
    echo "[✔] client smoke ok"
  else
    FAIL=1
    echo "[X] client smoke failed"
  fi
fi

if [[ "$MODE" == "all" ]]; then
  if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "[!] DATABASE_URL not set — skipping rust example smokes"
  else
    echo "=== [4] rust example smokes (batch) ==="
    for ex in swap_faucet_smoke withdrawal_smoke stake_lend_smoke merchant_admin_pubapi_smoke rewards_smoke; do
      echo "--- cargo run -p db --example $ex ---"
      cargo run -p db --example "$ex" || FAIL=1
    done
  fi
fi

echo
echo "======== RESUMO DO LOTE ========"
echo "mode: $MODE"
if [[ "$FAIL" -ne 0 ]]; then
  echo "status: FAILED"
  exit 1
fi
echo "status: OK"
echo "=== [audit] cargo+npm (best-effort) ==="
if command -v cargo-audit >/dev/null 2>&1; then cargo audit || true; else echo "(cargo-audit not installed)"; fi
(cd client && npm audit --audit-level=high || true)
echo
echo "[✔] lote completo passou"

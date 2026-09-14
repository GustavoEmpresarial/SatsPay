#!/usr/bin/env bash
# Low-RAM llvm-cov wrapper for ~14 GiB machines.
# Caps cargo jobs + test threads so rustc/llvm-cov don't thrash into swap.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-2}"
JOBS="${CARGO_BUILD_JOBS}"

if [[ -z "${DATABASE_URL:-}" ]]; then
  export DATABASE_URL="${DATABASE_URL:-postgresql://bitcosats:test@127.0.0.1:5439/bitcosats}"
fi

usage() {
  cat <<'EOF'
Usage: scripts/cov_lowmem.sh <scope> [extra cargo-llvm-cov args...]

Scopes:
  support   db::support + api-http::support only (fast)
  db        package db --lib --tests (fail-under 90)
  api-http  package api-http --lib --tests
  core      shared+crypto+domain (fail-under 90)
  all-gate  same packages as CI rust-llvm-cov gate (sequential)

Env:
  CARGO_BUILD_JOBS   default 2
  RUST_TEST_THREADS  default 2
  DATABASE_URL       default local sqlx-test on :5439
EOF
}

scope="${1:-}"
shift || true

run() {
  echo "[cov_lowmem] jobs=${JOBS} test_threads=${RUST_TEST_THREADS} :: $*"
  cargo llvm-cov -j "${JOBS}" "$@"
}

case "${scope}" in
  support)
    run --package db --lib --tests \
      --ignore-filename-regex '(admin|aave|airdrop|audit|auth|deposits|faucet|faucetlist|house|ledger|lend|merchant|merchant_deposits|network_fees|oauth|pricing|public_api|referral|rewards|stake|dex_swap|swap|telemetry|treasury_health|wallet|withdrawals|lib\.rs|tests/)' \
      --fail-under-lines 100 --summary-only "$@"
    run --package api-http --lib --tests \
      --ignore-filename-regex '(admin|airdrop|auth|client_ip|deposits|faucet|lend|merchant|merchant_deposits|middleware|notify_email|csrf|oauth|oauth_pkce|oauth_redirect|public_api|rate_limit|referral|rewards|stake|state|status|swap|wallet|withdrawals|lib\.rs|tests/)' \
      --fail-under-lines 100 --summary-only "$@"
    ;;
  db)
    run --package db --lib --tests --fail-under-lines 90 --summary-only "$@"
    ;;
  api-http)
    run --package api-http --lib --tests --summary-only "$@"
    ;;
  core)
    run --package shared --package crypto --package domain --lib --fail-under-lines 90 --summary-only "$@"
    ;;
  all-gate)
    run --package shared --package crypto --package domain --lib --fail-under-lines 90 --summary-only
    run --package pricing --package swapkit --package captcha --package queue --lib --tests --fail-under-lines 90 --summary-only
    run --package events --lib --tests --ignore-filename-regex '(producer|relay)\.rs$' --fail-under-lines 90 --summary-only
    run --package chain --lib --tests \
      --ignore-filename-regex '(rpc_client|real_client|bitcore_client|dgb_client|evm_client|sol_client)\.rs$' \
      --fail-under-lines 90 --summary-only
    run --package db --lib --tests --fail-under-lines 90 --summary-only
    ;;
  -h|--help|"" )
    usage
    exit 1
    ;;
  *)
    echo "Unknown scope: ${scope}" >&2
    usage
    exit 1
    ;;
esac

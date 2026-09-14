#!/usr/bin/env bash
# Exercise CloudNativePG PITR restore against a real S3-compatible bucket.
# Does NOT delete or mutate the source cluster — creates a parallel restore
# cluster for audit. Pattern matches docs/operations/backup-restore-postgres.md.
#
# Required env (fail-fast, no invented defaults):
#   S3_BACKUP_URI      — Barman destinationPath, e.g. s3://bucket/bitcosats-postgres
#   S3_ACCESS_KEY      — object-store access key id
#   S3_SECRET_KEY      — object-store secret access key
#   S3_ENDPOINT_URL    — object-store endpoint, e.g. http://minio.bitcosats.svc:9000
#   PITR_TARGET_TIME   — ISO-8601 UTC timestamp, e.g. 2026-08-30T03:14:00Z
#   KUBE_NAMESPACE     — namespace where the restore Cluster is applied
#
# Optional:
#   RESTORE_CLUSTER_NAME  — default bitcosats-postgres-pitr-restore
#   EXTERNAL_CLUSTER_NAME — externalClusters[].name in the restore CR
#                           (default bitcosats-postgres)
#   EXTERNAL_SERVER_NAME  — Barman serverName folder in the bucket
#                           (default: same as EXTERNAL_CLUSTER_NAME; must match
#                           the source Cluster name used when backing up)
#   RESTORE_INSTANCES     — default 1 (k3d-friendly; prod runbook uses 3)
#   RESTORE_STORAGE_SIZE  — default 2Gi (match source/dev size)
#   CRED_SECRET_NAME      — default postgres-pitr-restore-credentials
set -euo pipefail

require() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "error: required env $name is unset or empty" >&2
    exit 1
  fi
}

require S3_BACKUP_URI
require S3_ACCESS_KEY
require S3_SECRET_KEY
require S3_ENDPOINT_URL
require PITR_TARGET_TIME
require KUBE_NAMESPACE

RESTORE_CLUSTER_NAME="${RESTORE_CLUSTER_NAME:-bitcosats-postgres-pitr-restore}"
EXTERNAL_CLUSTER_NAME="${EXTERNAL_CLUSTER_NAME:-bitcosats-postgres}"
EXTERNAL_SERVER_NAME="${EXTERNAL_SERVER_NAME:-$EXTERNAL_CLUSTER_NAME}"
RESTORE_INSTANCES="${RESTORE_INSTANCES:-1}"
RESTORE_STORAGE_SIZE="${RESTORE_STORAGE_SIZE:-2Gi}"
CRED_SECRET_NAME="${CRED_SECRET_NAME:-postgres-pitr-restore-credentials}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE="${SCRIPT_DIR}/templates/pitr-restore-cluster.yaml.tmpl"

if [[ ! -f "$TEMPLATE" ]]; then
  echo "error: missing template $TEMPLATE" >&2
  exit 1
fi

if ! command -v envsubst >/dev/null 2>&1; then
  echo "error: envsubst is required (gettext)" >&2
  exit 1
fi

if ! command -v kubectl >/dev/null 2>&1; then
  echo "error: kubectl is required" >&2
  exit 1
fi

export S3_BACKUP_URI S3_ACCESS_KEY S3_SECRET_KEY S3_ENDPOINT_URL PITR_TARGET_TIME KUBE_NAMESPACE
export RESTORE_CLUSTER_NAME EXTERNAL_CLUSTER_NAME EXTERNAL_SERVER_NAME CRED_SECRET_NAME
export RESTORE_INSTANCES RESTORE_STORAGE_SIZE

echo "==> Creating/updating S3 credentials Secret ${CRED_SECRET_NAME} in ${KUBE_NAMESPACE}"
kubectl -n "$KUBE_NAMESPACE" create secret generic "$CRED_SECRET_NAME" \
  --from-literal=ACCESS_KEY_ID="$S3_ACCESS_KEY" \
  --from-literal=SECRET_ACCESS_KEY="$S3_SECRET_KEY" \
  --dry-run=client -o yaml | kubectl apply -f -

RENDERED="$(mktemp)"
trap 'rm -f "$RENDERED"' EXIT

echo "==> Rendering restore Cluster from runbook template (targetTime=${PITR_TARGET_TIME})"
envsubst '${S3_BACKUP_URI} ${S3_ACCESS_KEY} ${S3_SECRET_KEY} ${S3_ENDPOINT_URL} ${PITR_TARGET_TIME} ${KUBE_NAMESPACE} ${RESTORE_CLUSTER_NAME} ${EXTERNAL_CLUSTER_NAME} ${EXTERNAL_SERVER_NAME} ${CRED_SECRET_NAME} ${RESTORE_INSTANCES} ${RESTORE_STORAGE_SIZE}' \
  < "$TEMPLATE" > "$RENDERED"

echo "==> Applying restore Cluster ${RESTORE_CLUSTER_NAME} (source cluster is NOT deleted)"
kubectl apply -f "$RENDERED"

echo "==> Waiting for Cluster ${RESTORE_CLUSTER_NAME} to become Ready"
kubectl -n "$KUBE_NAMESPACE" wait \
  --for=condition=Ready \
  --timeout=600s \
  "cluster/${RESTORE_CLUSTER_NAME}"

cat <<EOF

PITR restore Cluster is Ready.

Next audit steps (do NOT point production traffic here yet):
  1. kubectl get cluster ${RESTORE_CLUSTER_NAME} -n ${KUBE_NAMESPACE}
  2. Port-forward the restored RW service and inspect ledger_entries /
     outbox_events against the expected pre-incident state
     (see docs/operations/backup-restore-postgres.md — "Depois de restaurar").
  3. Only after audit: cut DATABASE_URL to the restored app Secret and
     roll api-server/worker.
  4. Only then shut down the old cluster — this script never deletes it.

EOF

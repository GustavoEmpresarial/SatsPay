#!/usr/bin/env bash
# Exercise Postgres backup → restore on the Docker Compose stack (VM reality).
# Does NOT replace CloudNativePG/Barman PITR (scripts/exercise-pitr-restore.sh);
# that path needs k8s CNPG + S3. This script validates the *deployed* compose DR.
#
# Usage (on the compose host, from repo or /root/bitcosats):
#   ./scripts/exercise_compose_pg_backup_restore.sh
#
# Optional env:
#   COMPOSE_DIR   — default: directory containing docker-compose.yml (auto-detect)
#   PG_CONTAINER  — default: bitcosats-postgres
#   PGUSER/PGDATABASE — default: bitcosats
set -euo pipefail

PG_CONTAINER="${PG_CONTAINER:-bitcosats-postgres}"
PGUSER="${PGUSER:-bitcosats}"
PGDATABASE="${PGDATABASE:-bitcosats}"
CANARY_TAG="pitr_compose_canary_$(date -u +%Y%m%dT%H%M%SZ)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if ! docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  echo "error: container $PG_CONTAINER is not running" >&2
  exit 1
fi

echo "==> Inserting canary row tag=$CANARY_TAG"
docker exec -i "$PG_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -v ON_ERROR_STOP=1 <<SQL
CREATE TABLE IF NOT EXISTS _dr_exercise_canary (
  id bigserial PRIMARY KEY,
  tag text NOT NULL UNIQUE,
  created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO _dr_exercise_canary (tag) VALUES ('${CANARY_TAG}');
SQL

DUMP="$WORK/bitcosats.dump"
echo "==> pg_dump custom format → $DUMP"
docker exec "$PG_CONTAINER" pg_dump -U "$PGUSER" -d "$PGDATABASE" -Fc -f /tmp/bitcosats.dump
docker cp "$PG_CONTAINER:/tmp/bitcosats.dump" "$DUMP"
docker exec "$PG_CONTAINER" rm -f /tmp/bitcosats.dump

RESTORE_NAME="bitcosats-pg-restore-$$"
echo "==> Starting ephemeral restore container $RESTORE_NAME"
docker run -d --rm --name "$RESTORE_NAME" \
  -e POSTGRES_USER="$PGUSER" \
  -e POSTGRES_PASSWORD=restore_only \
  -e POSTGRES_DB="$PGDATABASE" \
  postgres:16-alpine >/dev/null

echo "==> Waiting for restore Postgres"
for i in $(seq 1 40); do
  if docker exec "$RESTORE_NAME" pg_isready -U "$PGUSER" -d "$PGDATABASE" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
docker exec "$RESTORE_NAME" pg_isready -U "$PGUSER" -d "$PGDATABASE"

docker cp "$DUMP" "$RESTORE_NAME:/tmp/bitcosats.dump"
echo "==> pg_restore into ephemeral DB"
docker exec "$RESTORE_NAME" pg_restore -U "$PGUSER" -d "$PGDATABASE" --clean --if-exists /tmp/bitcosats.dump \
  || true # --clean may warn on empty extensions; verify canary below

echo "==> Verifying canary on restore"
FOUND="$(docker exec "$RESTORE_NAME" psql -U "$PGUSER" -d "$PGDATABASE" -Atc \
  "SELECT tag FROM _dr_exercise_canary WHERE tag='${CANARY_TAG}'")"
if [[ "$FOUND" != "$CANARY_TAG" ]]; then
  echo "error: canary missing on restore (got '${FOUND}')" >&2
  docker stop "$RESTORE_NAME" >/dev/null || true
  exit 1
fi
echo "OK canary restored: $FOUND"

echo "==> Cleanup restore container + source canary"
docker stop "$RESTORE_NAME" >/dev/null
docker exec -i "$PG_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -v ON_ERROR_STOP=1 <<SQL
DELETE FROM _dr_exercise_canary WHERE tag='${CANARY_TAG}';
SQL

echo "==> Compose Postgres backup/restore exercise PASSED"

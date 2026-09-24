//! Fresh worker validation marker for the ADR 0012 custody split.

use sqlx::PgPool;

/// Worker heartbeat interval is 30s; two missed heartbeats make the API
/// unready and stop every new address issuance.
const VALID_FOR_SECONDS: i64 = 90;

pub async fn invalidate_signer_validation(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM custody_signer_validation WHERE singleton = TRUE")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_signer_validated(
    pool: &PgPool,
    config_fingerprint: &str,
    worker_version: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO custody_signer_validation \
         (singleton, config_fingerprint, worker_version, validated_at) \
         VALUES (TRUE, $1, $2, now()) \
         ON CONFLICT (singleton) DO UPDATE SET \
           config_fingerprint = EXCLUDED.config_fingerprint, \
           worker_version = EXCLUDED.worker_version, validated_at = now()",
    )
    .bind(config_fingerprint)
    .bind(worker_version)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn signer_validation_is_current(
    pool: &PgPool,
    config_fingerprint: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM custody_signer_validation \
         WHERE singleton = TRUE AND config_fingerprint = $1 \
           AND validated_at >= now() - make_interval(secs => $2))",
    )
    .bind(config_fingerprint)
    .bind(VALID_FOR_SECONDS as f64)
    .fetch_one(pool)
    .await
}

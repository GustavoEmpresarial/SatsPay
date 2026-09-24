use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn signer_marker_is_bound_to_config_and_freshness(pool: PgPool) {
    assert!(
        !db::custody::signer_validation_is_current(&pool, "config-a")
            .await
            .unwrap()
    );

    db::custody::mark_signer_validated(&pool, "config-a", "test-sha")
        .await
        .unwrap();
    assert!(db::custody::signer_validation_is_current(&pool, "config-a")
        .await
        .unwrap());
    assert!(
        !db::custody::signer_validation_is_current(&pool, "config-b")
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE custody_signer_validation SET validated_at = now() - interval '91 seconds'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        !db::custody::signer_validation_is_current(&pool, "config-a")
            .await
            .unwrap()
    );

    db::custody::invalidate_signer_validation(&pool)
        .await
        .unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM custody_signer_validation")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

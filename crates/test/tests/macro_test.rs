//! Tests for the #[terminusdb_test::test] macro.

use terminusdb_test::test;

/// Test with both client and spec parameters.
#[test(db = "macro_both_params")]
async fn test_with_both_params(client: _, spec: _) -> anyhow::Result<()> {
    // Verify we have a working client and spec
    assert!(spec.db.starts_with("macro_both_params"));
    assert_eq!(spec.branch, Some("main".to_string()));

    // Verify client works
    let dbs = client.list_databases_simple().await?;
    let found = dbs.iter().any(|db| {
        db.path
            .as_ref()
            .map(|p| p.contains(&spec.db))
            .unwrap_or(false)
    });
    assert!(found, "Test database should exist");

    Ok(())
}

/// Test with only client parameter.
#[test(db = "macro_client_only")]
async fn test_with_client_only(client: _) -> anyhow::Result<()> {
    // Just verify client works
    let _dbs = client.list_databases_simple().await?;
    Ok(())
}

/// Test with no parameters (rare but valid).
#[test(db = "macro_no_params")]
async fn test_with_no_params() -> anyhow::Result<()> {
    // This just verifies the macro handles no-param case
    Ok(())
}

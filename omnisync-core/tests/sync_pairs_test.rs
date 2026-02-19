use omnisync_core::SyncEngine;
use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::path::Path;

#[tokio::test]
async fn test_sync_pairs_management() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let engine = SyncEngine::new(pool);

    // Create a dummy directory to watch
    let test_dir = Path::new("test_sync_dir");
    if !test_dir.exists() {
        fs::create_dir(test_dir).unwrap();
    }
    let abs_path = fs::canonicalize(test_dir).unwrap();
    let abs_path_str = abs_path.to_str().unwrap();

    // Test adding a sync pair
    let id = engine
        .add_sync_pair(abs_path_str, "/remote/path", "mock_provider")
        .await
        .expect("Failed to add sync pair");

    assert!(id > 0);

    // Test retrieving sync pairs
    let pairs = engine.get_sync_pairs().await.expect("Failed to get pairs");
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].local_path, abs_path_str);
    assert_eq!(pairs[0].status, "active");

    // Clean up
    fs::remove_dir(test_dir).unwrap();
}

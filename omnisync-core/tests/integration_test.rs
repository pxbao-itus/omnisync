use omnisync_core::SyncEngine;
use sqlx::sqlite::SqlitePoolOptions;
use std::time::Duration;

#[tokio::test]
async fn test_engine_initialization() {
    // Use an in-memory database for testing
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Initialize the engine
    let engine = SyncEngine::new(pool);
    
    // Run start in a background task with a timeout to verify it starts and runs
    // Since start() loops indefinitely, a timeout expiration is actually a success (it didn't crash)
    let result = tokio::time::timeout(Duration::from_millis(500), engine.start()).await;
    
    // Check that it didn't return an error (it should have timed out)
    if let Ok(config_res) = result {
         assert!(config_res.is_ok(), "Engine returned error: {:?}", config_res.err());
    } else {
        // Timeout is good, it means it's running
    }
}

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
    
    // Just verify we can create it and it doesn't crash immediately
    // In a real scenario, we would mock providers and test sync logic
    assert!(engine.start().await.is_ok());
}

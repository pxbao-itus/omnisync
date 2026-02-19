use crate::provider::CloudProvider;
use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SyncEngine {
    pool: SqlitePool,
    providers: Vec<Arc<dyn CloudProvider>>,
    // We might want a way to manage active sync tasks, potentially a dashmap or similar
}

impl SyncEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            providers: Vec::new(),
        }
    }

    pub fn register_provider(&mut self, provider: Arc<dyn CloudProvider>) {
        self.providers.push(provider);
    }

    pub async fn start(&self) -> Result<()> {
        // Start the sync loop
        Ok(())
    }
}

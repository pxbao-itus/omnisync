use crate::models::SyncPair;
use crate::provider::CloudProvider;
use crate::watcher::FilesystemWatcher;
use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SyncEngine {
    pool: SqlitePool,
    providers: Vec<Arc<dyn CloudProvider>>,
    watcher: Arc<Mutex<FilesystemWatcher>>,
}

impl SyncEngine {
    pub fn new(pool: SqlitePool) -> Self {
        // Initialize watcher. In a real app, handle error better.
        let watcher = FilesystemWatcher::new().expect("Failed to initialize watcher");
        
        Self {
            pool,
            providers: Vec::new(),
            watcher: Arc::new(Mutex::new(watcher)),
        }
    }

    pub fn register_provider(&mut self, provider: Arc<dyn CloudProvider>) {
        self.providers.push(provider);
    }

    pub async fn start(&self) -> Result<()> {
        // 1. Load active sync pairs
        let pairs = self.get_sync_pairs().await?;
        
        // 2. Start watching paths
        {
            let mut watcher = self.watcher.lock().await;
            for pair in &pairs {
                if pair.status == "active" {
                    let path = Path::new(&pair.local_path);
                    if path.exists() {
                        watcher.watch(path).context("Failed to watch path")?;
                        println!("Watching: {:?}", path);
                    }
                }
            }
        }

        // 3. Start the event loop
        loop {
            // We lock the watcher briefly to check for events
            // In a real app we might want a separate channel or async-friendly watcher stream
            let event = {
                let watcher_guard = self.watcher.lock().await;
                watcher_guard.try_recv()
            };

            if let Some(event_result) = event {
                match event_result {
                    Ok(event) => {
                        println!("Detected change: {:?}", event);
                        // TODO: Queue this event for the TransferManager
                    }
                    Err(e) => eprintln!("Watch error: {:?}", e),
                }
            }
            
            // Sleep to prevent busy loop (this is a naive implementation for now)
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn add_sync_pair(&self, local: &str, remote: &str, provider: &str) -> Result<i64> {
        // Use query_scalar (non-macro) to avoid compile-time DB requirement
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO sync_pairs (local_path, remote_path, provider_id)
            VALUES (?, ?, ?)
            RETURNING id
            "#
        )
        .bind(local)
        .bind(remote)
        .bind(provider)
        .fetch_one(&self.pool)
        .await?;

        // Add to watcher immediately if active
        let mut watcher = self.watcher.lock().await;
        watcher.watch(Path::new(local))?;

        Ok(id)
    }

    pub async fn get_sync_pairs(&self) -> Result<Vec<SyncPair>> {
        // Use query_as (non-macro)
        let pairs = sqlx::query_as::<_, SyncPair>(
            r#"
            SELECT id, local_path, remote_path, provider_id, status, created_at
            FROM sync_pairs
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(pairs)
    }
}

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
            let event = {
                let watcher_guard = self.watcher.lock().await;
                watcher_guard.try_recv()
            };

            if let Some(event_result) = event {
                match event_result {
                    Ok(event) => {
                        use notify::EventKind;
                        let should_upload = matches!(
                            event.kind,
                            EventKind::Create(_) | EventKind::Modify(_)
                        );

                        if should_upload {
                            for changed_path in &event.paths {
                                println!("Detected change: {:?}", changed_path);

                                // Find the matching sync pair for this path
                                let pairs = self.get_sync_pairs().await.unwrap_or_default();
                                for pair in &pairs {
                                    if changed_path.starts_with(&pair.local_path) {
                                        // Build the remote path: replace local prefix with remote prefix
                                        let rel = changed_path
                                            .strip_prefix(&pair.local_path)
                                            .unwrap_or(changed_path);
                                        let remote_path = format!(
                                            "{}/{}",
                                            pair.remote_path.trim_end_matches('/'),
                                            rel.display()
                                        );

                                        // Trigger upload on all providers matching this pair
                                        for provider in &self.providers {
                                            if provider.id() == pair.provider_id {
                                                if let Err(e) = provider
                                                    .upload_file(changed_path, &remote_path)
                                                    .await
                                                {
                                                    eprintln!("Upload error: {:?}", e);
                                                } else {
                                                    println!(
                                                        "Uploaded {:?} -> {}",
                                                        changed_path, remote_path
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Watch error: {:?}", e),
                }
            }

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

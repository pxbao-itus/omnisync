use crate::models::SyncPair;
use crate::provider::{CloudProvider, CloudError};
use crate::watcher::FilesystemWatcher;
use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, Duration};
use rand::{Rng, thread_rng};
use sha2::{Sha256, Digest};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use async_recursion::async_recursion;
use futures::future::BoxFuture;
use futures::stream::StreamExt;

#[derive(Clone)]
pub struct SyncEngine {
    pool: SqlitePool,
    providers: Arc<Mutex<Vec<Arc<dyn CloudProvider>>>>,
    watcher: Arc<Mutex<FilesystemWatcher>>,
    sync_cache: Arc<Mutex<HashMap<PathBuf, Instant>>>,
    cancel_tokens: Arc<Mutex<HashMap<i64, Arc<std::sync::atomic::AtomicBool>>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum SyncStatus {
    Idle { pair_id: i64 },
    Syncing { pair_id: i64, path: String },
    Downloading { pair_id: i64, path: String },
    Uploaded { pair_id: i64, path: String },
    Deleted { pair_id: i64, path: String },
    Error { pair_id: i64, path: String, message: String },
    AuthExpired { account_id: String },
}

/// Extract provider type from account_id (e.g., "gdrive:user@gmail.com" -> "gdrive")
fn provider_type(account_id: &str) -> &str {
    account_id.split(':').next().unwrap_or(account_id)
}

/// Build a provider instance from credentials
fn make_provider(account_id: &str, access_token: String) -> Option<Box<dyn CloudProvider>> {
    match provider_type(account_id) {
        "gdrive" => Some(Box::new(crate::providers::gdrive::GoogleDriveProvider::new(access_token))),
        _ => None,
    }
}

impl SyncEngine {
    pub fn new(pool: SqlitePool) -> Self {
        let watcher = FilesystemWatcher::new().expect("Failed to initialize watcher");

        Self {
            pool,
            providers: Arc::new(Mutex::new(Vec::new())),
            watcher: Arc::new(Mutex::new(watcher)),
            sync_cache: Arc::new(Mutex::new(HashMap::new())),
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register_provider(&self, provider: Arc<dyn CloudProvider>) {
        self.providers.lock().await.push(provider);
    }

    pub async fn start<F>(&self, on_status: F) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        // 1. Load active sync pairs
        let pairs = self.get_sync_pairs().await?;
        let on_status = Arc::new(on_status);

        // 2. Initial sync for all active pairs
        for pair in &pairs {
            if pair.status == "active" {
                let path = Path::new(&pair.local_path);
                if path.exists() {
                    let mut watcher = self.watcher.lock().await;
                    watcher.watch(path).context("Failed to watch path")?;
                    println!("Watching: {:?}", path);
                    drop(watcher);
                    
                    let _ = self.perform_initial_sync(pair, on_status.clone()).await;
                }
            }
        }

        // 3. Start the event loop with periodic background poll
        let mut last_poll = Instant::now();
        let mut last_token_refresh = Instant::now();
        
        loop {
            // Collect events for a short period to group them
            let mut pending_paths: HashMap<PathBuf, notify::EventKind> = HashMap::new();
            
            // Drain current channel
            while let Some(event_result) = {
                let watcher_guard = self.watcher.lock().await;
                watcher_guard.try_recv()
            } {
                if let Ok(event) = event_result {
                    for path in event.paths {
                        pending_paths.insert(path, event.kind);
                    }
                }
            }

            if !pending_paths.is_empty() {
                let pairs = self.get_sync_pairs().await.unwrap_or_default();
                for (path, kind) in pending_paths {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" { continue; }
                    }

                    for pair in &pairs {
                        if pair.status == "active" && path.starts_with(&pair.local_path) {
                            let cancel = {
                                let tokens = self.cancel_tokens.lock().await;
                                tokens.get(&pair.id).cloned()
                            };
                            
                            if let Some(c) = &cancel {
                                if c.load(std::sync::atomic::Ordering::Relaxed) { continue; }
                            }

                            match kind {
                                notify::EventKind::Remove(_) => {
                                    println!("Watcher: Detected removal of {:?}", path);
                                    if let Err(e) = self.delete_remote_file(&path, pair, on_status.clone(), cancel).await {
                                        eprintln!("Failed to sync deletion for {:?}: {:?}", path, e);
                                    } else {
                                        println!("Successfully synced deletion for {:?}", path);
                                    }
                                }
                                notify::EventKind::Modify(m) => {
                                    if !matches!(m, notify::event::ModifyKind::Any | notify::event::ModifyKind::Data(_) | notify::event::ModifyKind::Metadata(_)) {
                                        continue;
                                    }
                                    if path.exists() {
                                        if path.is_dir() {
                                            let creds = self.get_valid_credentials(&pair.account_id).await.unwrap_or(None);
                                            if let Some(creds) = creds {
                                                if let Some(provider) = make_provider(&pair.account_id, creds.access_token) {
                                                    let _ = self.ensure_remote_path_exists(provider.as_ref(), pair, &path).await;
                                                }
                                            }
                                        } else {
                                            let _ = self.sync_file(&path, pair, on_status.clone(), cancel).await;
                                        }
                                    }
                                }
                                notify::EventKind::Create(_) => {
                                    if path.exists() {
                                        if path.is_dir() {
                                            let creds = self.get_valid_credentials(&pair.account_id).await.unwrap_or(None);
                                            if let Some(creds) = creds {
                                                if let Some(provider) = make_provider(&pair.account_id, creds.access_token) {
                                                    let _ = self.ensure_remote_path_exists(provider.as_ref(), pair, &path).await;
                                                }
                                            }
                                        } else {
                                            let _ = self.sync_file(&path, pair, on_status.clone(), cancel).await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            // Periodic token refresh (every 30 min) — keeps all Google OAuth sessions alive
            if last_token_refresh.elapsed() > Duration::from_secs(30 * 60) {
                println!("Proactive token refresh check...");
                if let Ok(accounts) = self.get_accounts_for_provider("gdrive").await {
                    for account in &accounts {
                        match self.get_valid_credentials(&account.account_id).await {
                            Ok(Some(_)) => {
                                println!("Token refreshed successfully for {}", account.account_id);
                            }
                            Ok(None) => {}
                            Err(e) => {
                                eprintln!("Token refresh failed for {}: {}", account.account_id, e);
                            }
                        }
                    }
                }
                last_token_refresh = Instant::now();
            }

            // Periodic cloud poll (every 60s)
            if last_poll.elapsed() > Duration::from_secs(60) {
                let pairs = self.get_sync_pairs().await.unwrap_or_default();
                for pair in &pairs {
                    if pair.status == "active" {
                        // Spawn background sync to avoid blocking the event loop
                        let engine = self.clone();
                        let pair_c = pair.clone();
                        let on_status_c = on_status.clone();
                        tokio::spawn(async move {
                            let _ = engine.perform_initial_sync(&pair_c, on_status_c).await;
                        });
                    }
                }
                last_poll = Instant::now();
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    pub async fn sync_file<F>(&self, path: &Path, pair: &SyncPair, on_status: Arc<F>, cancel: Option<Arc<std::sync::atomic::AtomicBool>>) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        if let Some(c) = &cancel {
            if c.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
        }
        // 1. Debounce check
        {
            let mut cache = self.sync_cache.lock().await;
            if let Some(last) = cache.get(path) {
                if last.elapsed() < Duration::from_secs(5) {
                    return Ok(());
                }
            }
            cache.insert(path.to_path_buf(), Instant::now());
        }

        let creds = self.get_valid_credentials(&pair.account_id).await.unwrap_or(None);
        if let Some(creds) = creds {
            let provider = match make_provider(&pair.account_id, creds.access_token) {
                Some(p) => p,
                None => return Ok(()),
            };

            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

            let parent = path.parent().ok_or_else(|| anyhow::anyhow!("No parent"))?;
            let remote_parent_id = match self.ensure_remote_path_exists(provider.as_ref(), pair, parent).await {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Failed to resolve remote parent for {:?}: {:?}", path, e);
                    return Err(e);
                }
            };

            // Pre-check: Does it actually need syncing?
            let local_meta = tokio::fs::metadata(path).await?;
            let local_size = local_meta.len();
            let local_hash = self.compute_local_hash(path).await?;
            
            let existing_info = provider.list_files(&remote_parent_id).await?;
            if let Some(remote) = existing_info.iter().find(|r| r.name == filename) {
                let matches = if let Some(r_hash) = &remote.hash {
                    *r_hash == local_hash
                } else if let Some(r_size) = remote.size {
                    r_size == local_size
                } else {
                    false
                };

                if matches {
                    return Ok(());
                }
            }

            let path_str = path.to_string_lossy().to_string();
            let pair_id = pair.id;
            on_status(SyncStatus::Syncing { pair_id, path: path_str.clone() });

            if let Err(e) = provider.upload_file(path, &remote_parent_id).await {
                eprintln!("Upload error for {:?}: {:?}", path, e);
                on_status(SyncStatus::Error { pair_id, path: path_str.clone(), message: e.to_string() });
                if matches!(e, CloudError::Unauthenticated) {
                    eprintln!("Token expired during upload, disconnecting account: {}", pair.account_id);
                    let _ = self.disconnect_account(&pair.account_id).await;
                    on_status(SyncStatus::AuthExpired { account_id: pair.account_id.clone() });
                }
                return Err(e.into());
            } else {
                println!("Successfully synced {:?} -> folder ID {}", path, pair.remote_path);
                on_status(SyncStatus::Uploaded { pair_id, path: path_str.clone() });

                {
                    let mut cache = self.sync_cache.lock().await;
                    cache.insert(path.to_path_buf(), Instant::now());
                }

                let on_status_clone = on_status.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    on_status_clone(SyncStatus::Idle { pair_id });
                });
            }
        }
        Ok(())
    }

    pub async fn perform_initial_sync<F>(&self, pair: &SyncPair, on_status: Arc<F>) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        let local_path = Path::new(&pair.local_path);
        if !local_path.exists() { return Ok(()); }

        let token = {
            let mut tokens = self.cancel_tokens.lock().await;
            if let Some(old) = tokens.remove(&pair.id) {
                old.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let t = Arc::new(std::sync::atomic::AtomicBool::new(false));
            tokens.insert(pair.id, t.clone());
            t
        };

        let _token_c = token.clone();

        let creds = self.get_valid_credentials(&pair.account_id).await?.ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let provider: Arc<dyn CloudProvider> = match provider_type(&pair.account_id) {
            "gdrive" => Arc::new(crate::providers::gdrive::GoogleDriveProvider::new(creds.access_token)),
            _ => return Ok(()),
        };

        self.sync_directory_recursive(local_path, &pair.remote_path, pair, &provider, &on_status, token).await?;

        // Update last sync time
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        sqlx::query("UPDATE sync_pairs SET last_sync_at = ? WHERE id = ?")
            .bind(now)
            .bind(pair.id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    #[async_recursion]
    async fn sync_directory_recursive<F>(
        &self,
        local_dir: &Path,
        remote_dir_id: &str,
        pair: &SyncPair,
        provider: &Arc<dyn CloudProvider>,
        on_status: &Arc<F>,
        cancel: Arc<std::sync::atomic::AtomicBool>
    ) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
             return Ok(());
        }


        // 1. Get local files & dirs
        let mut local_entries = HashMap::new();
        let mut entries = tokio::fs::read_dir(local_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            local_entries.insert(name, entry.path());
        }

        // 2. Get remote files & dirs
        let remote_entries = provider.list_files(remote_dir_id).await?;

        let mut tasks: Vec<BoxFuture<'_, Result<()>>> = Vec::new();

        // 3. Reconcile: Local to Cloud
        for (name, path) in &local_entries {
            if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" { continue; }

            if let Some(remote) = remote_entries.iter().find(|r| &r.name == name) {
                if path.is_dir() {
                    if remote.is_dir {
                        let path = path.clone();
                        let remote_id = remote.id.clone();
                        let cancel_c = cancel.clone();
                        tasks.push(Box::pin(async move {
                            if cancel_c.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
                            self.sync_directory_recursive(&path, &remote_id, pair, provider, on_status, cancel_c).await
                        }));
                    }
                } else {
                    if !remote.is_dir {
                        let path = path.clone();
                        let remote_id = remote.id.clone();
                        let remote_size = remote.size;
                        let remote_mtime = remote.modified_at;
                        let remote_hash = remote.hash.clone();
                        let cancel_c = cancel.clone();
                        let on_status_c = on_status.clone();
                        tasks.push(Box::pin(async move {
                            if cancel_c.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
                            let local_meta = tokio::fs::metadata(&path).await?;
                            let local_size = local_meta.len();
                            let local_mtime = local_meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

                            match (remote_size, remote_mtime, remote_hash) {
                                (Some(_r_size), Some(r_mtime), Some(r_hash)) => {
                                    let local_hash = self.compute_local_hash(&path).await?;
                                    if local_hash != r_hash {
                                        if local_mtime > r_mtime + 2 {
                                            println!("Sync: Local file {:?} is newer and content differs. Uploading.", path);
                                            let _ = self.sync_file(&path, pair, on_status_c, Some(cancel_c)).await;
                                        } else if r_mtime > local_mtime + 2 {
                                            println!("Sync: Cloud file {:?} is newer and content differs. Downloading.", path);
                                            let _ = self.sync_remote_to_local(&remote_id, &path, pair, on_status_c, Some(cancel_c)).await;
                                        }
                                    }
                                }
                                (Some(r_size), Some(r_mtime), None) => {
                                    if local_mtime > r_mtime + 2 {
                                        println!("Sync: Local file {:?} is newer (no cloud hash). Uploading.", path);
                                        let _ = self.sync_file(&path, pair, on_status_c, Some(cancel_c)).await;
                                    } else if r_mtime > local_mtime + 2 || local_size != r_size {
                                        println!("Sync: Cloud file {:?} is newer or size differs (no cloud hash). Downloading.", path);
                                        let _ = self.sync_remote_to_local(&remote_id, &path, pair, on_status_c, Some(cancel_c)).await;
                                    }
                                }
                                _ => {
                                    println!("Sync: Missing metadata for {:?}, checking via sync_file.", path);
                                    let _ = self.sync_file(&path, pair, on_status_c, Some(cancel_c)).await;
                                }
                            };
                            Ok(())
                        }));
                    }
                }
            } else {
                // Missing in cloud
                let path = path.clone();
                let name = name.clone();
                let remote_dir_id = remote_dir_id.to_string();
                let on_status_c = on_status.clone();
                let cancel_c = cancel.clone();
                tasks.push(Box::pin(async move {
                    if cancel_c.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
                    let local_meta = tokio::fs::metadata(&path).await?;
                    let local_mtime = local_meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
                    
                    let mut was_there_before = false;
                    if let Some(last_sync) = pair.last_sync_at {
                        if local_mtime < last_sync {
                            was_there_before = true;
                        }
                    }

                    if path.is_dir() {
                        if was_there_before {
                            println!("Directory {:?} missing on cloud (was there before), deleting locally", path);
                            let _ = tokio::fs::remove_dir_all(&path).await;
                        } else {
                            let new_folder_id = provider.create_folder(&name, &remote_dir_id).await?;
                            self.sync_directory_recursive(&path, &new_folder_id, pair, provider, &on_status_c, cancel_c).await?;
                        }
                    } else {
                        if was_there_before {
                            println!("File {:?} missing on cloud (was there before), deleting locally", path);
                            let path_str = path.to_string_lossy().to_string();
                            let pair_id = pair.id;
                            if let Err(e) = tokio::fs::remove_file(&path).await {
                                eprintln!("Failed to delete local file {:?}: {:?}", path, e);
                            } else {
                                on_status_c(SyncStatus::Deleted { pair_id, path: path_str });
                            }
                        } else {
                            let _ = self.sync_file(&path, pair, on_status_c, Some(cancel_c)).await;
                        }
                    }
                    Ok(())
                }));
            }
        }

        // 4. Reconcile: Cloud to Local
        for remote in &remote_entries {
            if remote.name.starts_with('.') || remote.name == "node_modules" || remote.name == "target" || remote.name == "dist" { continue; }
            if !local_entries.contains_key(&remote.name) {
                let dest = local_dir.join(&remote.name);
                let remote_name = remote.name.clone();
                let remote_id = remote.id.clone();
                let remote_is_dir = remote.is_dir;
                let remote_modified_at = remote.modified_at;
                let remote_dir_id = remote_dir_id.to_string();
                let on_status_c = on_status.clone();
                let cancel_c = cancel.clone();
                tasks.push(Box::pin(async move {
                    if cancel_c.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
                    let mut was_deleted_locally = false;
                    if let (Some(last_sync), Some(r_mtime)) = (pair.last_sync_at, remote_modified_at) {
                        if r_mtime <= last_sync {
                            was_deleted_locally = true;
                        }
                    }

                    if was_deleted_locally {
                        println!("Sync: File {:?} missing locally (was there before), deleting on cloud", remote_name);
                        if let Err(e) = provider.delete_file(&remote_name, &remote_dir_id).await {
                            eprintln!("Failed to sync local deletion to cloud for {}: {:?}", remote_name, e);
                        }
                    } else {
                        if remote_is_dir {
                            tokio::fs::create_dir_all(&dest).await?;
                            self.sync_directory_recursive(&dest, &remote_id, pair, provider, &on_status_c, cancel_c).await?;
                        } else {
                            println!("Sync: File {:?} is new on cloud. Downloading.", remote_name);
                            let _ = self.sync_remote_to_local(&remote_id, &dest, pair, on_status_c, Some(cancel_c)).await;
                        }
                    }
                    Ok(())
                }));
            }
        }

        // Run concurrently (up to 3 parallel file transfers / directory syncs)
        let mut stream = futures::stream::iter(tasks).buffer_unordered(3);
        while let Some(res) = stream.next().await {
            if let Err(e) = res {
                eprintln!("Directory sync error: {:?}", e);
            }
        }

        Ok(())
    }

    async fn ensure_remote_path_exists(&self, provider: &dyn CloudProvider, pair: &SyncPair, local_path: &Path) -> Result<String> {
        let relative = local_path.strip_prefix(&pair.local_path)?;
        let mut current_id = pair.remote_path.clone();
        
        for component in relative.components() {
            let name = component.as_os_str().to_string_lossy();
            if name == "." || name == "/" { continue; }

            let entries = provider.list_files(&current_id).await?;
            if let Some(folder) = entries.iter().find(|e| e.name == name && e.is_dir) {
                current_id = folder.id.clone();
            } else {
                current_id = provider.create_folder(&name, &current_id).await?;
            }
        }
        
        Ok(current_id)
    }

    pub async fn sync_remote_to_local<F>(&self, file_id: &str, dest: &Path, pair: &SyncPair, on_status: Arc<F>, cancel: Option<Arc<std::sync::atomic::AtomicBool>>) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        if let Some(c) = &cancel {
            if c.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
        }
        let creds = self.get_valid_credentials(&pair.account_id).await?.ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let provider = match make_provider(&pair.account_id, creds.access_token) {
            Some(p) => p,
            None => return Ok(()),
        };

        let path_str = dest.to_string_lossy().to_string();
        let pair_id = pair.id;
        on_status(SyncStatus::Downloading { pair_id, path: path_str.clone() });

        if let Err(e) = provider.download_file(file_id, dest).await {
            eprintln!("Download error: {:?}", e);
            on_status(SyncStatus::Error { pair_id, path: path_str.clone(), message: e.to_string() });
            return Err(e.into());
        } else {
            println!("Successfully downloaded -> {:?}", dest);
            
            {
                let mut cache = self.sync_cache.lock().await;
                cache.insert(dest.to_path_buf(), Instant::now());
            }

            on_status(SyncStatus::Uploaded { pair_id, path: path_str.clone() });
            let on_status_clone = on_status.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                on_status_clone(SyncStatus::Idle { pair_id });
            });
        }
        Ok(())
    }

    // ---- Data Operations ----

    pub async fn add_sync_pair(&self, local: &str, remote: &str, remote_name: &str, provider: &str, account_id: &str) -> Result<i64> {
        let canonical_local = Path::new(local).canonicalize()?.to_string_lossy().to_string();
        
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO sync_pairs (local_path, remote_path, remote_name, provider_id, account_id)
            VALUES (?, ?, ?, ?, ?)
            RETURNING id
            "#
        )
        .bind(&canonical_local)
        .bind(remote)
        .bind(remote_name)
        .bind(provider)
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;

        let mut watcher = self.watcher.lock().await;
        watcher.watch(Path::new(&canonical_local))?;

        Ok(id)
    }

    pub async fn remove_sync_pair(&self, id: i64) -> Result<()> {
        let pair: Option<SyncPair> = sqlx::query_as::<_, SyncPair>(
            "SELECT id, local_path, remote_path, remote_name, provider_id, account_id, status, created_at, last_sync_at FROM sync_pairs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(pair) = pair {
            let mut watcher = self.watcher.lock().await;
            // Check if any other pair still uses this path before unwatching
            let pairs = self.get_sync_pairs().await.unwrap_or_default();
            let other_exists = pairs.iter().any(|p| p.id != id && p.local_path == pair.local_path);
            if !other_exists {
                let _ = watcher.unwatch(Path::new(&pair.local_path));
            }
        }

        {
            let mut tokens = self.cancel_tokens.lock().await;
            if let Some(token) = tokens.remove(&id) {
                token.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        sqlx::query("DELETE FROM sync_pairs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_sync_pairs(&self) -> Result<Vec<SyncPair>> {
        let pairs = sqlx::query_as::<_, SyncPair>(
            r#"
            SELECT id, local_path, remote_path, remote_name, provider_id, account_id, status, created_at, last_sync_at
            FROM sync_pairs
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(pairs)
    }

    // ---- Credential Operations (multi-account) ----

    pub async fn set_credentials(
        &self, 
        account_id: &str,
        provider_id: &str,
        access_token: &str, 
        refresh_token: Option<&str>, 
        expires_in: Option<i64>,
        user_name: Option<String>,
        user_email: Option<String>,
        user_avatar: Option<String>,
    ) -> Result<()> {
        let expires_at = expires_in.map(|secs| (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64) + secs);
        
        sqlx::query(
            "INSERT OR REPLACE INTO credentials (account_id, provider_id, access_token, refresh_token, expires_at, user_name, user_email, user_avatar) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(account_id)
        .bind(provider_id)
        .bind(access_token)
        .bind(refresh_token)
        .bind(expires_at)
        .bind(user_name)
        .bind(user_email)
        .bind(user_avatar)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_credentials(&self, account_id: &str) -> Result<Option<crate::models::Credentials>> {
        let creds: Option<crate::models::Credentials> = sqlx::query_as(
            "SELECT account_id, provider_id, access_token, refresh_token, expires_at, user_name, user_email, user_avatar FROM credentials WHERE account_id = ?"
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(creds)
    }

    pub async fn get_valid_credentials(&self, account_id: &str) -> Result<Option<crate::models::Credentials>> {
        let creds = self.get_credentials(account_id).await?;
        if let Some(mut creds) = creds {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
            
            // If expires within 10 minutes, refresh proactively
            if let Some(expires_at) = creds.expires_at {
                if expires_at - now < 600 {
                    let ptype = provider_type(account_id);
                    if ptype == "gdrive" {
                        if let Some(refresh_token) = &creds.refresh_token {
                            println!("Refreshing Google token for {} (expires in {}s)...", account_id, expires_at - now);
                            match self.refresh_google_token(refresh_token).await {
                                Ok((new_access, new_expires)) => {
                                    let mut user_name = creds.user_name.clone();
                                    let mut user_email = creds.user_email.clone();
                                    let mut user_avatar = creds.user_avatar.clone();
                                    
                                    if user_name.is_none() {
                                        if let Ok((n, e, a)) = self.fetch_google_user_info(&new_access).await {
                                            user_name = n;
                                            user_email = e;
                                            user_avatar = a;
                                        }
                                    }

                                    self.set_credentials(account_id, "gdrive", &new_access, Some(refresh_token), new_expires, user_name.clone(), user_email.clone(), user_avatar.clone()).await?;
                                    creds.access_token = new_access;
                                    creds.expires_at = new_expires.map(|secs| now + secs);
                                    creds.user_name = user_name;
                                    creds.user_email = user_email;
                                    creds.user_avatar = user_avatar;
                                    return Ok(Some(creds));
                                }
                                Err(e) => {
                                    eprintln!("Failed to refresh token for {}: {}", account_id, e);
                                    if expires_at > now {
                                        eprintln!("Token still valid for {}s, using current token", expires_at - now);
                                        return Ok(Some(creds));
                                    }
                                    // Token fully expired — disconnect
                                    eprintln!("Token fully expired and refresh failed, disconnecting account: {}", account_id);
                                    let _ = self.disconnect_account(account_id).await;
                                    return Ok(None);
                                }
                            }
                        }
                    }
                }
            }
            return Ok(Some(creds));
        }
        Ok(None)
    }

    async fn refresh_google_token(&self, refresh_token: &str) -> Result<(String, Option<i64>)> {
        let client_id = crate::config::get_google_client_id();
        let client_secret = crate::config::get_google_client_secret();
        
        let client = reqwest::Client::new();
        let params = [
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let response = client.post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let err = response.text().await?;
            return Err(anyhow::anyhow!("Token refresh failed: {}", err));
        }

        let tokens: serde_json::Value = response.json().await?;
        let access_token = tokens["access_token"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No access token returned"))?.to_string();
        let expires_in = tokens["expires_in"].as_i64();
        
        Ok((access_token, expires_in))
    }

    async fn fetch_google_user_info(&self, access_token: &str) -> Result<(Option<String>, Option<String>, Option<String>)> {
        let client = reqwest::Client::new();
        let response = client.get("https://www.googleapis.com/drive/v3/about?fields=user")
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok((None, None, None));
        }

        let body: serde_json::Value = response.json().await?;
        let user = &body["user"];
        
        let name = user["displayName"].as_str().map(|s| s.to_string());
        let email = user["emailAddress"].as_str().map(|s| s.to_string());
        let avatar = user["photoLink"].as_str().map(|s| s.to_string());
        
        Ok((name, email, avatar))
    }

    // ---- Account Management (multi-account) ----

    pub async fn disconnect_account(&self, account_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM credentials WHERE account_id = ?")
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get all connected accounts
    pub async fn get_all_accounts(&self) -> Result<Vec<crate::models::Credentials>> {
        let accounts = sqlx::query_as::<_, crate::models::Credentials>(
            "SELECT account_id, provider_id, access_token, refresh_token, expires_at, user_name, user_email, user_avatar FROM credentials ORDER BY account_id"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(accounts)
    }

    /// Get all connected accounts for a specific provider
    pub async fn get_accounts_for_provider(&self, provider_id: &str) -> Result<Vec<crate::models::Credentials>> {
        let accounts = sqlx::query_as::<_, crate::models::Credentials>(
            "SELECT account_id, provider_id, access_token, refresh_token, expires_at, user_name, user_email, user_avatar FROM credentials WHERE provider_id = ? ORDER BY account_id"
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(accounts)
    }

    pub async fn get_remote_folders(&self, account_id: &str) -> Result<Vec<crate::provider::RemoteFolder>> {
        let creds = self.get_valid_credentials(account_id).await?
            .ok_or_else(|| anyhow::anyhow!("Account not connected"))?;

        let ptype = provider_type(account_id);
        if ptype == "gdrive" {
            let provider = crate::providers::gdrive::GoogleDriveProvider::new(creds.access_token);
            match provider.list_folders().await {
                Ok(folders) => Ok(folders),
                Err(CloudError::Unauthenticated) => {
                    eprintln!("Authentication failed, disconnecting account: {}", account_id);
                    let _ = self.disconnect_account(account_id).await;
                    Err(CloudError::Unauthenticated.into())
                }
                Err(e) => Err(e.into()),
            }
        } else {
            Err(anyhow::anyhow!("Provider not supported yet"))
        }
    }

    pub fn generate_pkce() -> (String, String) {
        let mut rng = thread_rng();
        let verifier: String = (0..64)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect();

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

        (verifier, challenge)
    }

    /// Returns the account_id of the authenticated account (e.g., "gdrive:user@gmail.com")
    pub async fn authenticate_google(&self, client_id: &str, client_secret: &str, code_verifier: String) -> Result<String> {
        let redirect_uri = "http://127.0.0.1:4420";
        let auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=https://www.googleapis.com/auth/drive&access_type=offline&prompt=consent",
            client_id, redirect_uri
        );

        let listener = TcpListener::bind("127.0.0.1:4420").await?;
        println!("Please visit this URL to authenticate: {}", auth_url);
        
        let (mut socket, _) = listener.accept().await?;
        
        let mut buffer = [0; 4096];
        let n = socket.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..n]);
        
        let code = request.lines().next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|path| {
                let url = url::Url::parse(&format!("http://127.0.0.1{}", path)).ok()?;
                url.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.into_owned())
            })
            .ok_or_else(|| anyhow::anyhow!("Failed to extract authorization code"))?;

        let client = reqwest::Client::new();
        let params = [
            ("code", code.as_str()),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", &code_verifier),
        ];

        let response = client.post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let err = response.text().await?;
            return Err(anyhow::anyhow!("Token exchange failed: {}", err));
        }

        let tokens: serde_json::Value = response.json().await?;
        let access_token = tokens["access_token"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No access token returned"))?;
        let refresh_token = tokens["refresh_token"].as_str();
        let expires_in = tokens["expires_in"].as_i64();
        
        // Fetch user info to build account_id
        let (user_name, user_email, user_avatar) = self.fetch_google_user_info(access_token).await.unwrap_or((None, None, None));
        
        let email = user_email.clone().unwrap_or_else(|| "unknown".to_string());
        let account_id = format!("gdrive:{}", email);
        
        self.set_credentials(&account_id, "gdrive", access_token, refresh_token, expires_in, user_name, user_email, user_avatar).await?;

        let response_body = "<html><body style='font-family:sans-serif;text-align:center;padding-top:50px;'><h1>✅ Authentication Successful!</h1><p>OmniSync is now connected. You can close this window now.</p></body></html>";
        let response_http = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(response_http.as_bytes()).await?;
        socket.flush().await?;

        Ok(account_id)
    }

    pub async fn delete_remote_file<F>(&self, path: &Path, pair: &SyncPair, on_status: Arc<F>, cancel: Option<Arc<std::sync::atomic::AtomicBool>>) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        if let Some(c) = &cancel {
            if c.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
        }
        let creds = self.get_valid_credentials(&pair.account_id).await.unwrap_or(None);
        if let Some(creds) = creds {
            let provider = match make_provider(&pair.account_id, creds.access_token) {
                Some(p) => p,
                None => return Ok(()),
            };

            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

            let parent = path.parent().ok_or_else(|| anyhow::anyhow!("No parent"))?;
            let remote_parent_id = match self.ensure_remote_path_exists(provider.as_ref(), pair, parent).await {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Failed to resolve remote parent for deletion {:?}: {:?}", path, e);
                    return Err(e);
                }
            };

            let path_str = path.to_string_lossy().to_string();
            let pair_id = pair.id;
            
            if let Err(e) = provider.delete_file(filename, &remote_parent_id).await {
                eprintln!("Delete error: {:?}", e);
                if matches!(e, CloudError::Unauthenticated) {
                    eprintln!("Token expired during delete, disconnecting account: {}", pair.account_id);
                    let _ = self.disconnect_account(&pair.account_id).await;
                    on_status(SyncStatus::AuthExpired { account_id: pair.account_id.clone() });
                }
                return Err(e.into());
            } else {
                on_status(SyncStatus::Deleted { pair_id, path: path_str.clone() });
                
                let on_status_clone = on_status.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    on_status_clone(SyncStatus::Idle { pair_id });
                });
            }
        }
        Ok(())
    }

    async fn compute_local_hash(&self, path: &Path) -> Result<String> {
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = md5::Context::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 { break; }
            hasher.consume(&buffer[..n]);
        }
        let digest = hasher.compute();
        Ok(format!("{:x}", digest))
    }
}

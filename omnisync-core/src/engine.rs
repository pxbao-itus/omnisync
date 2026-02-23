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

pub struct SyncEngine {
    pool: SqlitePool,
    providers: Mutex<Vec<Arc<dyn CloudProvider>>>,
    watcher: Arc<Mutex<FilesystemWatcher>>,
    sync_cache: Mutex<HashMap<PathBuf, Instant>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum SyncStatus {
    Idle,
    Syncing { path: String },
    Downloading { path: String },
    Uploaded { path: String },
    Deleted { path: String },
    Error { path: String, message: String },
}

impl SyncEngine {
    pub fn new(pool: SqlitePool) -> Self {
        // Initialize watcher. In a real app, handle error better.
        let watcher = FilesystemWatcher::new().expect("Failed to initialize watcher");

        Self {
            pool,
            providers: Mutex::new(Vec::new()),
            watcher: Arc::new(Mutex::new(watcher)),
            sync_cache: Mutex::new(HashMap::new()),
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
        
        loop {
            // Check for filesystem events
            while let Some(event_result) = {
                let watcher_guard = self.watcher.lock().await;
                watcher_guard.try_recv()
            } {
                match event_result {
                    Ok(event) => {
                        println!("Watch event: {:?} for paths {:?}", event.kind, event.paths);
                        for path in &event.paths {
                            if path.is_dir() && path.exists() { continue; }
                            
                            let pairs = self.get_sync_pairs().await.unwrap_or_default();
                            for pair in &pairs {
                                if path.starts_with(&pair.local_path) {
                                    if path.exists() {
                                        let _ = self.sync_file(path, pair, on_status.clone()).await;
                                    } else {
                                        let _ = self.delete_remote_file(path, pair, on_status.clone()).await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Watch error: {:?}", e),
                }
            }

            // Periodic cloud poll (every 30s)
            if last_poll.elapsed() > Duration::from_secs(30) {
                last_poll = Instant::now();
                println!("Performing periodic cloud poll...");
                let pairs = self.get_sync_pairs().await.unwrap_or_default();
                for pair in &pairs {
                    if pair.status == "active" {
                        let _ = self.perform_initial_sync(pair, on_status.clone()).await;
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn sync_file<F>(&self, path: &Path, pair: &SyncPair, on_status: Arc<F>) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        // 1. Debounce check: Avoid multiple syncs for the same file in short period (2s)
        {
            let mut cache = self.sync_cache.lock().await;
            if let Some(last) = cache.get(path) {
                if last.elapsed() < Duration::from_secs(2) {
                    return Ok(());
                }
            }
            cache.insert(path.to_path_buf(), Instant::now());
        }

        let token = self.get_credentials(&pair.provider_id).await.unwrap_or(None);
        if let Some(token) = token {
            let provider: Box<dyn CloudProvider> = if pair.provider_id == "gdrive" {
                Box::new(crate::providers::gdrive::GoogleDriveProvider::new(token))
            } else {
                return Ok(());
            };

            let path_str = path.to_string_lossy().to_string();
            on_status(SyncStatus::Syncing { path: path_str.clone() });

            if let Err(e) = provider.upload_file(path, &pair.remote_path).await {
                eprintln!("Upload error for {:?}: {:?}", path, e);
                on_status(SyncStatus::Error { path: path_str.clone(), message: e.to_string() });
                if matches!(e, CloudError::Unauthenticated) {
                    let _ = self.disconnect_provider(&pair.provider_id).await;
                }
                return Err(e.into());
            } else {
                println!("Successfully synced {:?} -> folder ID {}", path, pair.remote_path);
                on_status(SyncStatus::Uploaded { path: path_str.clone() });

                let on_status_clone = on_status.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    on_status_clone(SyncStatus::Idle);
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

        println!("Performing bidirectional sync for folder: {:?}", local_path);

        // 1. Get local files
        let mut local_files = HashMap::new();
        let mut entries = tokio::fs::read_dir(local_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                local_files.insert(name, entry.path());
            }
        }

        // 2. Get remote files
        let token = self.get_credentials(&pair.provider_id).await?.ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let provider: Box<dyn CloudProvider> = if pair.provider_id == "gdrive" {
            Box::new(crate::providers::gdrive::GoogleDriveProvider::new(token))
        } else {
            return Ok(());
        };
        let remote_files = provider.list_files(&pair.remote_path).await?;

        // 3. Reconcile
        // Local to Cloud (Upload if missing in Cloud)
        for (name, path) in &local_files {
            if !remote_files.iter().any(|f| &f.name == name) {
                let _ = self.sync_file(path, pair, on_status.clone()).await;
            }
        }

        // Cloud to Local (Download if missing in Local)
        for remote_file in &remote_files {
            if !local_files.contains_key(&remote_file.name) {
                let dest = local_path.join(&remote_file.name);
                let _ = self.sync_remote_to_local(&remote_file.id, &dest, pair, on_status.clone()).await;
            }
        }

        Ok(())
    }

    pub async fn sync_remote_to_local<F>(&self, file_id: &str, dest: &Path, pair: &SyncPair, on_status: Arc<F>) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        let token = self.get_credentials(&pair.provider_id).await?.ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let provider: Box<dyn CloudProvider> = if pair.provider_id == "gdrive" {
            Box::new(crate::providers::gdrive::GoogleDriveProvider::new(token))
        } else {
            return Ok(());
        };

        let path_str = dest.to_string_lossy().to_string();
        on_status(SyncStatus::Downloading { path: path_str.clone() });

        if let Err(e) = provider.download_file(file_id, dest).await {
            eprintln!("Download error: {:?}", e);
            on_status(SyncStatus::Error { path: path_str.clone(), message: e.to_string() });
            return Err(e.into());
        } else {
            println!("Successfully downloaded -> {:?}", dest);
            
            // Register in sync_cache to avoid immediate re-upload
            {
                let mut cache = self.sync_cache.lock().await;
                cache.insert(dest.to_path_buf(), Instant::now());
            }

            on_status(SyncStatus::Uploaded { path: path_str.clone() });
            let on_status_clone = on_status.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                on_status_clone(SyncStatus::Idle);
            });
        }
        Ok(())
    }

    pub async fn add_sync_pair(&self, local: &str, remote: &str, provider: &str) -> Result<i64> {
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

        let mut watcher = self.watcher.lock().await;
        watcher.watch(Path::new(local))?;

        Ok(id)
    }

    pub async fn remove_sync_pair(&self, id: i64) -> Result<()> {
        let pair: Option<SyncPair> = sqlx::query_as::<_, SyncPair>(
            "SELECT id, local_path, remote_path, provider_id, status, created_at FROM sync_pairs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(pair) = pair {
            let mut watcher = self.watcher.lock().await;
            let _ = watcher.unwatch(Path::new(&pair.local_path));
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
            SELECT id, local_path, remote_path, provider_id, status, created_at
            FROM sync_pairs
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(pairs)
    }

    pub async fn set_credentials(&self, provider_id: &str, access_token: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO credentials (provider_id, access_token) VALUES (?, ?)"
        )
        .bind(provider_id)
        .bind(access_token)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_credentials(&self, provider_id: &str) -> Result<Option<String>> {
        let token: Option<String> = sqlx::query_scalar(
            "SELECT access_token FROM credentials WHERE provider_id = ?"
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(token)
    }

    pub async fn disconnect_provider(&self, provider_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM credentials WHERE provider_id = ?")
            .bind(provider_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_remote_folders(&self, provider_id: &str) -> Result<Vec<crate::provider::RemoteFolder>> {
        let token = self.get_credentials(provider_id).await?
            .ok_or_else(|| anyhow::anyhow!("Provider not connected"))?;

        if provider_id == "gdrive" {
            let provider = crate::providers::gdrive::GoogleDriveProvider::new(token);
            match provider.list_folders().await {
                Ok(folders) => Ok(folders),
                Err(CloudError::Unauthenticated) => {
                    eprintln!("Authentication failed, disconnecting provider: {}", provider_id);
                    let _ = self.disconnect_provider(provider_id).await;
                    Err(CloudError::Unauthenticated.into())
                }
                Err(e) => Err(e.into()),
            }
        } else {
            Err(anyhow::anyhow!("Provider not supported yet"))
        }
    }

    pub async fn authenticate_google(&self, client_id: &str, client_secret: &str) -> Result<()> {
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
        
        self.set_credentials("gdrive", access_token).await?;

        let response_body = "<html><body style='font-family:sans-serif;text-align:center;padding-top:50px;'><h1>✅ Authentication Successful!</h1><p>OmniSync is now connected. You can close this window now.</p></body></html>";
        let response_http = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(response_http.as_bytes()).await?;
        socket.flush().await?;

        Ok(())
    }

    pub async fn delete_remote_file<F>(&self, path: &Path, pair: &SyncPair, on_status: Arc<F>) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
        let token = self.get_credentials(&pair.provider_id).await.unwrap_or(None);
        if let Some(token) = token {
            let provider: Box<dyn CloudProvider> = if pair.provider_id == "gdrive" {
                Box::new(crate::providers::gdrive::GoogleDriveProvider::new(token))
            } else {
                return Ok(());
            };

            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

            let path_str = path.to_string_lossy().to_string();
            
            if let Err(e) = provider.delete_file(filename, &pair.remote_path).await {
                eprintln!("Delete error: {:?}", e);
                if matches!(e, CloudError::Unauthenticated) {
                    let _ = self.disconnect_provider(&pair.provider_id).await;
                }
                return Err(e.into());
            } else {
                on_status(SyncStatus::Deleted { path: path_str.clone() });
                
                let on_status_clone = on_status.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    on_status_clone(SyncStatus::Idle);
                });
            }
        }
        Ok(())
    }
}

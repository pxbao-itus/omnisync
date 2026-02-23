use crate::models::SyncPair;
use crate::provider::CloudProvider;
use crate::watcher::FilesystemWatcher;
use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct SyncEngine {
    pool: SqlitePool,
    providers: Vec<Arc<dyn CloudProvider>>,
    watcher: Arc<Mutex<FilesystemWatcher>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum SyncStatus {
    Idle,
    Syncing { path: String },
    Uploaded { path: String },
    Error { path: String, message: String },
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

    pub async fn start<F>(&mut self, on_status: F) -> Result<()>
    where
        F: Fn(SyncStatus) + Send + Sync + 'static,
    {
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

        let on_status = Arc::new(on_status);

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
                                if changed_path.is_dir() { continue; }
                                
                                println!("Detected change: {:?}", changed_path);

                                // Find the matching sync pair for this path
                                let pairs = self.get_sync_pairs().await.unwrap_or_default();
                                for pair in &pairs {
                                    if changed_path.starts_with(&pair.local_path) {
                                        // Build the remote path
                                        let rel = changed_path
                                            .strip_prefix(&pair.local_path)
                                            .unwrap_or(changed_path);
                                        let remote_path = format!(
                                            "{}/{}",
                                            pair.remote_path.trim_end_matches('/'),
                                            rel.display()
                                        );

                                        // Resolve provider dynamically based on current credentials
                                        let token = self.get_credentials(&pair.provider_id).await.unwrap_or(None);
                                        
                                        if let Some(token) = token {
                                            let provider: Box<dyn CloudProvider> = if pair.provider_id == "gdrive" {
                                                Box::new(crate::providers::gdrive::GoogleDriveProvider::new(token))
                                            } else {
                                                continue;
                                            };

                                            let path_str = changed_path.to_string_lossy().to_string();
                                            on_status(SyncStatus::Syncing { path: path_str.clone() });

                                            if let Err(e) = provider.upload_file(changed_path, &remote_path).await {
                                                eprintln!("Upload error: {:?}", e);
                                                on_status(SyncStatus::Error { path: path_str.clone(), message: e.to_string() });
                                            } else {
                                                println!("Uploaded {:?} -> {}", changed_path, remote_path);
                                                on_status(SyncStatus::Uploaded { path: path_str.clone() });
                                            }
                                            
                                            let on_status_clone = on_status.clone();
                                            tokio::spawn(async move {
                                                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                                                on_status_clone(SyncStatus::Idle);
                                            });
                                        } else {
                                            eprintln!("No credentials for provider: {}", pair.provider_id);
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

    pub async fn remove_sync_pair(&self, id: i64) -> Result<()> {
        // Get the pair first so we can unwatch the path
        let pair: Option<SyncPair> = sqlx::query_as::<_, SyncPair>(
            "SELECT id, local_path, remote_path, provider_id, status, created_at FROM sync_pairs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(pair) = pair {
            // Unwatch the path
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

        // Instantiate provider on the fly for this request
        if provider_id == "gdrive" {
            let provider = crate::providers::gdrive::GoogleDriveProvider::new(token);
            provider.list_folders().await
        } else {
            Err(anyhow::anyhow!("Provider not supported yet"))
        }
    }

    pub async fn authenticate_google(&self, client_id: &str, client_secret: &str) -> Result<()> {
        let redirect_uri = "http://localhost:4420";
        let auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=https://www.googleapis.com/auth/drive&access_type=offline&prompt=consent",
            client_id, redirect_uri
        );

        // Start local server to listen for the code
        let listener = TcpListener::bind("127.0.0.1:4420").await?;
        println!("Please visit this URL to authenticate: {}", auth_url);
        
        // Open browser (handled by Tauri frontend caller usually, but we can print it)
        // In this implementation, we wait for one connection
        let (mut socket, _) = listener.accept().await?;
        
        let mut buffer = [0; 1024];
        let n = socket.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..n]);
        
        // Extract code from GET /?code=...
        let code = request.lines().next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|path| {
                let url = url::Url::parse(&format!("http://localhost{}", path)).ok()?;
                url.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.into_owned())
            })
            .ok_or_else(|| anyhow::anyhow!("Failed to extract authorization code"))?;

        // Exchange code for tokens
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
        
        // Save to DB
        self.set_credentials("gdrive", access_token).await?;

        // Send a response to the browser
        let response_body = "<html><body><h1>Authentication Successful!</h1><p>You can close this window now.</p></body></html>";
        let response_http = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(response_http.as_bytes()).await?;

        Ok(())
    }
}

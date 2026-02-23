use omnisync_core::SyncEngine;
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::Mutex;

// Shared app state
struct AppState {
    engine: Arc<SyncEngine>,
}

#[derive(Debug, Clone, Serialize)]
struct SyncPairResponse {
    id: i64,
    local_path: String,
    remote_path: String,
    remote_name: String,
    provider_id: String,
    status: String,
    created_at: i64,
}

#[tauri::command]
async fn get_sync_pairs(state: State<'_, AppState>) -> Result<Vec<SyncPairResponse>, String> {
    let pairs = state.engine
        .get_sync_pairs()
        .await
        .map_err(|e| format!("Failed to get sync pairs: {}", e))?;

    Ok(pairs
        .into_iter()
        .map(|p| SyncPairResponse {
            id: p.id,
            local_path: p.local_path,
            remote_path: p.remote_path,
            remote_name: p.remote_name,
            provider_id: p.provider_id,
            status: p.status,
            created_at: p.created_at,
        })
        .collect())
}

#[tauri::command]
async fn add_sync_pair(
    state: State<'_, AppState>,
    local_path: String,
    remote_path: String,
    remote_name: String,
    provider_id: String,
) -> Result<i64, String> {
    let id = state.engine
        .add_sync_pair(&local_path, &remote_path, &remote_name, &provider_id)
        .await
        .map_err(|e| format!("Failed to add sync pair: {}", e))?;
    Ok(id)
}

#[tauri::command]
async fn remove_sync_pair(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.engine
        .remove_sync_pair(id)
        .await
        .map_err(|e| format!("Failed to remove sync pair: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn connect_provider(state: State<'_, AppState>, provider_id: String, token: String) -> Result<(), String> {
    state.engine
        .set_credentials(&provider_id, &token)
        .await
        .map_err(|e| format!("Failed to connect provider: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn list_remote_folders(state: State<'_, AppState>, provider_id: String) -> Result<Vec<omnisync_core::provider::RemoteFolder>, String> {
    state.engine
        .get_remote_folders(&provider_id)
        .await
        .map_err(|e| format!("Failed to list remote folders: {}", e))?
        .pipe(Ok)
}

#[tauri::command]
async fn get_auth_status(state: State<'_, AppState>, provider_id: String) -> Result<bool, String> {
    let token = state.engine
        .get_credentials(&provider_id)
        .await
        .map_err(|e| format!("Failed to get auth status: {}", e))?;
    Ok(token.is_some())
}

#[tauri::command]
async fn disconnect_provider(state: State<'_, AppState>, provider_id: String) -> Result<(), String> {
    state.engine
        .disconnect_provider(&provider_id)
        .await
        .map_err(|e| format!("Failed to disconnect provider: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn start_oauth(app: tauri::AppHandle, state: State<'_, AppState>, provider_id: String) -> Result<(), String> {
    if provider_id != "gdrive" {
        return Err("Provider not supported".to_string());
    }

    use tauri_plugin_shell::ShellExt;
    let client_id = omnisync_core::config::get_google_client_id();
    let client_secret = omnisync_core::config::get_google_client_secret();

    if client_id == "NOT_CONFIGURED" {
        return Err("Google OAuth Client ID not configured in .env file".to_string());
    }

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri=http://127.0.0.1:4420&response_type=code&scope=https://www.googleapis.com/auth/drive&access_type=offline&prompt=consent",
        client_id
    );

    // Start the auth listener in the background *before* opening the browser
    let engine = state.engine.clone();
    let auth_handle = tauri::async_runtime::spawn(async move {
        engine.authenticate_google(&client_id, &client_secret).await
    });

    // Short sleep to give the background task time to bind the TcpListener
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Open browser effectively
    app.shell().open(auth_url, None).map_err(|e| e.to_string())?;

    // Now wait for the background task to finish the auth flow
    auth_handle.await
        .map_err(|e| format!("Auth task panicked: {}", e))?
        .map_err(|e| format!("Authentication failed: {}", e))?;
    
    Ok(())
}

trait Pipe {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T where Self: Sized;
}

impl<T> Pipe for T {
    fn pipe<U>(self, f: impl FnOnce(Self) -> U) -> U {
        f(self)
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            // Initialize database and pool synchronously during setup
            let pool = tauri::async_runtime::block_on(async {
                let db_path = app_handle
                    .path()
                    .app_data_dir()
                    .expect("Failed to get app data dir");
                std::fs::create_dir_all(&db_path).ok();
                let db_file = db_path.join("omnisync.db");

                let connection_options =
                    SqliteConnectOptions::from_str(&format!("sqlite://{}", db_file.display()))
                        .expect("Invalid DB path")
                        .create_if_missing(true);

                let pool = SqlitePoolOptions::new()
                    .connect_with(connection_options)
                    .await
                    .expect("Failed to connect to database");

                sqlx::migrate!("../../omnisync-core/migrations")
                    .run(&pool)
                    .await
                    .expect("Failed to run migrations");

                pool
            });

            let engine = Arc::new(SyncEngine::new(pool));
            let engine_clone = engine.clone();

            // Manage state immediately so commands can use it
            app.manage(AppState {
                engine,
            });

            // Start engine loop in background
            tauri::async_runtime::spawn(async move {
                if let Err(e) = engine_clone.start(move |status| {
                    use tauri::Emitter;
                    let _ = app_handle.emit("sync-status", status);
                }).await {
                    eprintln!("Engine error: {:?}", e);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sync_pairs,
            add_sync_pair,
            remove_sync_pair,
            connect_provider,
            list_remote_folders,
            get_auth_status,
            start_oauth,
            disconnect_provider
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

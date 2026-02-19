use anyhow::Result;
use clap::{Parser, Subcommand};
use omnisync_core::{providers::gdrive::GoogleDriveProvider, SyncEngine};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "./omnisync.db")]
    db_path: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the sync daemon
    Daemon,
    /// Login to a cloud provider
    Login {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        token: String,
    },
    /// Add a sync pair
    Add {
        #[arg(long)]
        local: String,
        #[arg(long)]
        remote: String,
        #[arg(long)]
        provider: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let connection_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", args.db_path))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(connection_options)
        .await?;

    sqlx::migrate!("../omnisync-core/migrations")
        .run(&pool)
        .await?;

    let mut engine = SyncEngine::new(pool.clone());

    // Load credentials and initialize providers
    // In a real app, we'd check which providers are configured
    // For MVP, if we have a gdrive token in DB, we init the provider
    let gdrive_token: Option<String> = sqlx::query_scalar(
        "SELECT access_token FROM credentials WHERE provider_id = 'gdrive'"
    )
    .fetch_optional(&pool)
    .await?;

    if let Some(token) = gdrive_token {
        println!("Initializing Google Drive provider...");
        let provider = GoogleDriveProvider::new(token);
        engine.register_provider(Arc::new(provider));
    }

    match args.command {
        Commands::Daemon => {
            println!("Starting OmniSync Daemon...");
            // TODO: In real app, we should only start if there are providers
            engine.start().await?;
        }
        Commands::Login { provider, token } => {
            if provider != "gdrive" {
                return Err(anyhow::anyhow!("Only 'gdrive' is supported for now"));
            }
            
            let mut query = sqlx::query("INSERT INTO credentials (provider_id, access_token) VALUES (?, ?) ON CONFLICT(provider_id) DO UPDATE SET access_token = excluded.access_token");
            query = query.bind(&provider).bind(token);
            query.execute(&pool).await?;
            
            println!("Successfully logged in to {}", provider);
        }
        Commands::Add { local, remote, provider } => {
            let id = engine.add_sync_pair(&local, &remote, &provider).await?;
            println!("Added sync pair with ID: {}", id);
        }
    }

    Ok(())
}

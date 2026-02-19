use anyhow::Result;
use clap::Parser;
use omnisync_core::SyncEngine;
use sqlx::sqlite::SqlitePoolOptions;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    db_path: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let pool = SqlitePoolOptions::new()
        .connect(&format!("sqlite:{}", args.db_path))
        .await?;

    sqlx::migrate!("../omnisync-core/migrations")
        .run(&pool)
        .await?;

    let engine = SyncEngine::new(pool);
    println!("OmniSync Engine initialized.");

    Ok(())
}

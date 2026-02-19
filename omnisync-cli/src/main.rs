use anyhow::Result;
use clap::Parser;
use omnisync_core::SyncEngine;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    db_path: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Create the database file if it doesn't exist
    let connection_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", args.db_path))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(connection_options)
        .await?;

    sqlx::migrate!("../omnisync-core/migrations")
        .run(&pool)
        .await?;

    let _engine = SyncEngine::new(pool);
    println!("OmniSync Engine initialized.");

    Ok(())
}

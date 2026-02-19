use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SyncPair {
    pub id: i64,
    pub local_path: String,
    pub remote_path: String,
    pub provider_id: String,
    pub status: String,
    pub created_at: i64,
}

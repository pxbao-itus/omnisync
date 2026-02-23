use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SyncPair {
    pub id: i64,
    pub local_path: String,
    pub remote_path: String,
    pub remote_name: String,
    pub provider_id: String,
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Credentials {
    pub provider_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub user_avatar: Option<String>,
}

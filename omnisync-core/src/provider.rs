use async_trait::async_trait;
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum CloudError {
    #[error("Authentication failed (401)")]
    Unauthenticated,
    #[error("API Error: {0}")]
    ApiError(String),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type CloudResult<T> = std::result::Result<T, CloudError>;

#[async_trait]
pub trait CloudProvider: Send + Sync {
    /// return the identifier of the provider (e.g., "gdrive", "onedrive")
    fn id(&self) -> &str;

    /// Upload a file to the cloud
    async fn upload_file(&self, local_path: &Path, cloud_path: &str) -> CloudResult<()>;

    /// Download a file from the cloud
    async fn download_file(&self, file_id: &str, local_path: &Path) -> CloudResult<()>;
    
    /// Delete a file on the cloud
    async fn delete_file(&self, filename: &str, cloud_parent: &str) -> CloudResult<()>;
    
    /// Get metadata for a file (hash, size, modified_at)
    async fn get_metadata(&self, cloud_path: &str) -> CloudResult<FileMetadata>;

    /// List files in a specific folder
    async fn list_files(&self, folder_id: &str) -> CloudResult<Vec<RemoteFile>>;

    /// List folders in the cloud
    async fn list_folders(&self) -> CloudResult<Vec<RemoteFolder>>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteFolder {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub hash: String,
    pub size: u64,
    pub modified_at: i64,
}

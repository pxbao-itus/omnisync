use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait CloudProvider: Send + Sync {
    /// return the identifier of the provider (e.g., "gdrive", "onedrive")
    fn id(&self) -> &str;

    /// Upload a file to the cloud
    async fn upload_file(&self, local_path: &Path, cloud_path: &str) -> Result<()>;

    /// Download a file from the cloud
    async fn download_file(&self, cloud_path: &str, local_path: &Path) -> Result<()>;
    
    /// Delete a file on the cloud
    async fn delete_file(&self, cloud_path: &str) -> Result<()>;
    
    /// Get metadata for a file (hash, size, modified_at)
    async fn get_metadata(&self, cloud_path: &str) -> Result<FileMetadata>;
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub hash: String,
    pub size: u64,
    pub modified_at: i64,
}

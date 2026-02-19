use crate::provider::{CloudProvider, FileMetadata};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub struct GoogleDriveProvider {
    client: Client,
    access_token: String,
}

impl GoogleDriveProvider {
    pub fn new(access_token: String) -> Self {
        Self {
            client: Client::new(),
            access_token,
        }
    }
}

#[async_trait]
impl CloudProvider for GoogleDriveProvider {
    fn id(&self) -> &str {
        "gdrive"
    }

    async fn upload_file(&self, local_path: &Path, _cloud_path: &str) -> Result<()> {
        let mut file = File::open(local_path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;

        let filename = local_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid filename"))?;

        // Simple multipart upload (metadata + media) is ideal, but for MVP 
        // we'll try simple upload or just creating the file.
        // Google Drive API v3: POST https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart
        
        // For this MVP, let's use the 'simple' uploadType if we just want content, 
        // but typically we need metadata (name, parent).
        // Let's do a simple multipart request using reqwest.
        
        let metadata_part = serde_json::json!({
            "name": filename,
            // "parents": ["root"] // Optional: handle parents later
        })
        .to_string();

        let form = reqwest::multipart::Form::new()
            .part("metadata", reqwest::multipart::Part::text(metadata_part).mime_str("application/json")?)
            .part("file", reqwest::multipart::Part::bytes(contents).mime_str("application/octet-stream")?);

        let response = self.client
            .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
            .bearer_auth(&self.access_token)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Upload failed: {}", error_text));
        }

        println!("Uploaded {} to Google Drive", filename);
        Ok(())
    }

    async fn download_file(&self, _cloud_path: &str, _local_path: &Path) -> Result<()> {
        // Implement later
        Ok(())
    }

    async fn delete_file(&self, _cloud_path: &str) -> Result<()> {
         // Implement later
        Ok(())
    }

    async fn get_metadata(&self, _cloud_path: &str) -> Result<FileMetadata> {
        // Implement later
        Ok(FileMetadata {
            hash: "dummy".to_string(),
            size: 0,
            modified_at: 0,
        })
    }
}

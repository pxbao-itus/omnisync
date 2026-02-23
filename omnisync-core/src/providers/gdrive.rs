use crate::provider::{CloudProvider, FileMetadata, CloudError, CloudResult};
use anyhow::anyhow;
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

    async fn upload_file(&self, local_path: &Path, _cloud_path: &str) -> CloudResult<()> {
        let mut file = File::open(local_path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;

        let filename = local_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| CloudError::Other(anyhow!("Invalid filename")))?;

        let metadata_part = serde_json::json!({
            "name": filename,
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

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthenticated);
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(CloudError::ApiError(error_text));
        }

        println!("Uploaded {} to Google Drive", filename);
        Ok(())
    }

    async fn download_file(&self, _cloud_path: &str, _local_path: &Path) -> CloudResult<()> {
        Ok(())
    }

    async fn delete_file(&self, _cloud_path: &str) -> CloudResult<()> {
        Ok(())
    }

    async fn get_metadata(&self, _cloud_path: &str) -> CloudResult<FileMetadata> {
        Ok(FileMetadata {
            hash: "dummy".to_string(),
            size: 0,
            modified_at: 0,
        })
    }

    async fn list_folders(&self) -> CloudResult<Vec<crate::provider::RemoteFolder>> {
        let response = self.client
            .get("https://www.googleapis.com/drive/v3/files")
            .query(&[
                ("q", "mimeType='application/vnd.google-apps.folder' and trashed=false"),
                ("fields", "files(id, name)"),
            ])
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthenticated);
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(CloudError::ApiError(error_text));
        }

        let body: serde_json::Value = response.json().await?;
        let files = body["files"].as_array().ok_or_else(|| CloudError::ApiError("Invalid response body".to_string()))?;

        let folders = files.iter().filter_map(|f| {
            let id = f["id"].as_str()?.to_string();
            let name = f["name"].as_str()?.to_string();
            Some(crate::provider::RemoteFolder { id, name })
        }).collect();

        Ok(folders)
    }
}

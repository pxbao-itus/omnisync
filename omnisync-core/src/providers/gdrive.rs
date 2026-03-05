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

    async fn find_file_info(&self, name: &str, parent_id: &str) -> CloudResult<Option<(String, Option<u64>, Option<i64>, Option<String>)>> {
        let escaped_name = name.replace("'", "\\'");
        let q = format!("name = '{}' and '{}' in parents and trashed = false", escaped_name, parent_id);
        let response = self.client
            .get("https://www.googleapis.com/drive/v3/files")
            .query(&[("q", q.as_str()), ("fields", "files(id, size, modifiedTime, md5Checksum)")])
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthenticated);
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(CloudError::ApiError(format!("Search failed: {}", error_text)));
        }

        let body: serde_json::Value = response.json().await?;
        let files = body["files"].as_array().ok_or_else(|| CloudError::ApiError("Invalid search response".to_string()))?;
        
        if let Some(f) = files.first() {
            let id = f["id"].as_str().map(|s| s.to_string());
            let size = f["size"].as_str().and_then(|s| s.parse().ok());
            let hash = f["md5Checksum"].as_str().map(|s| s.to_string());
            let modified_at = f["modifiedTime"].as_str().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp())
            });
            if let Some(id) = id {
                return Ok(Some((id, size, modified_at, hash)));
            }
        }
        Ok(None)
    }

    async fn compute_local_hash(&self, path: &Path) -> CloudResult<String> {
        let mut file = File::open(path).await?;
        let mut hasher = md5::Context::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 { break; }
            hasher.consume(&buffer[..n]);
        }
        let digest = hasher.compute();
        Ok(format!("{:x}", digest))
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

        // Check if file already exists
        let local_meta = tokio::fs::metadata(local_path).await?;
        let local_size = local_meta.len();
        
        // Get file timestamps for preserving dates
        let modified_time = local_meta.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default().to_rfc3339());
        let created_time = local_meta.created().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default().to_rfc3339());
        
        // Compute hash for more reliable comparison
        let local_hash = self.compute_local_hash(local_path).await?;

        let existing_info = self.find_file_info(filename, _cloud_path).await?;

        if let Some((file_id, r_size, _r_mtime, r_hash)) = existing_info {
            // Compare hash if available, otherwise fallback to size
            let matches = if let Some(hash) = r_hash {
                hash == local_hash
            } else if let Some(size) = r_size {
                local_size == size
            } else {
                false
            };

            if matches {
                return Ok(());
            }

            // Update existing file with metadata (to preserve modifiedTime)
            let mut metadata = serde_json::json!({});
            if let Some(ref mtime) = modified_time {
                metadata["modifiedTime"] = serde_json::json!(mtime);
            }

            let metadata_part = metadata.to_string();
            let form = reqwest::multipart::Form::new()
                .part("metadata", reqwest::multipart::Part::text(metadata_part).mime_str("application/json")?)
                .part("file", reqwest::multipart::Part::bytes(contents).mime_str("application/octet-stream")?);

            let response = self.client
                .patch(format!("https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=multipart", file_id))
                .bearer_auth(&self.access_token)
                .multipart(form)
                .send()
                .await?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(CloudError::Unauthenticated);
            }

            if !response.status().is_success() {
                let error_text = response.text().await?;
                return Err(CloudError::ApiError(format!("Update failed: {}", error_text)));
            }
            println!("Updated {} on Google Drive", filename);
        } else {
            // Create new file
            let mut metadata: serde_json::Value = serde_json::json!({
                "name": filename,
            });

            if _cloud_path != "root" && !_cloud_path.is_empty() {
                metadata["parents"] = serde_json::json!([_cloud_path]);
            }
            if let Some(ref ctime) = created_time {
                metadata["createdTime"] = serde_json::json!(ctime);
            }
            if let Some(ref mtime) = modified_time {
                metadata["modifiedTime"] = serde_json::json!(mtime);
            }

            let metadata_part = metadata.to_string();
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
                return Err(CloudError::ApiError(format!("Upload failed: {}", error_text)));
            }
            println!("Created {} on Google Drive", filename);
        }

        Ok(())
    }

    async fn download_file(&self, file_id: &str, local_path: &Path) -> CloudResult<()> {
        let response = self.client
            .get(format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id))
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthenticated);
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(CloudError::ApiError(format!("Download failed: {}", error_text)));
        }

        let bytes = response.bytes().await?;
        tokio::fs::write(local_path, bytes).await?;
        
        Ok(())
    }

    async fn create_folder(&self, name: &str, parent_id: &str) -> CloudResult<String> {
        // Check if exists
        if let Some((existing_id, _, _, _)) = self.find_file_info(name, parent_id).await? {
            return Ok(existing_id);
        }

        let metadata = serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
            "parents": [parent_id]
        });

        let response = self.client
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(&self.access_token)
            .json(&metadata)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthenticated);
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(CloudError::ApiError(format!("Create folder failed: {}", error_text)));
        }

        let body: serde_json::Value = response.json().await?;
        let id = body["id"].as_str().ok_or_else(|| CloudError::ApiError("No ID returned".to_string()))?.to_string();
        
        println!("Created folder {} on Google Drive", name);
        Ok(id)
    }

    async fn delete_file(&self, filename: &str, cloud_parent: &str) -> CloudResult<()> {
        let existing_info = self.find_file_info(filename, cloud_parent).await?;

        if let Some((file_id, _, _, _)) = existing_info {
            // DELETE https://www.googleapis.com/drive/v3/files/{fileId}
            let response = self.client
                .delete(format!("https://www.googleapis.com/drive/v3/files/{}", file_id))
                .bearer_auth(&self.access_token)
                .send()
                .await?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(CloudError::Unauthenticated);
            }

            if !response.status().is_success() {
                let error_text = response.text().await?;
                return Err(CloudError::ApiError(format!("Delete failed: {}", error_text)));
            }
            println!("Deleted {} from Google Drive", filename);
        }

        Ok(())
    }

    async fn list_files(&self, folder_id: &str) -> CloudResult<Vec<crate::provider::RemoteFile>> {
        let q = format!("'{}' in parents and trashed = false", folder_id);
        let mut all_files = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut request = self.client
                .get("https://www.googleapis.com/drive/v3/files")
                .query(&[("q", q.as_str()), ("fields", "nextPageToken, files(id, name, mimeType, size, modifiedTime, md5Checksum)"), ("pageSize", "1000")])
                .bearer_auth(&self.access_token);

            if let Some(token) = &page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }

            let response = request.send().await?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(CloudError::Unauthenticated);
            }

            if !response.status().is_success() {
                let error_text = response.text().await?;
                return Err(CloudError::ApiError(format!("List files failed: {}", error_text)));
            }

            let body: serde_json::Value = response.json().await?;
            let files_json = body["files"].as_array().ok_or_else(|| CloudError::ApiError("Invalid body".to_string()))?;
            
            for f in files_json {
                let id = f["id"].as_str().unwrap_or_default().to_string();
                let name = f["name"].as_str().unwrap_or_default().to_string();
                let mime_type = f["mimeType"].as_str().unwrap_or_default();
                let is_dir = mime_type == "application/vnd.google-apps.folder";
                let size = f["size"].as_str().and_then(|s| s.parse().ok());
                let hash = f["md5Checksum"].as_str().map(|s| s.to_string());
                let modified_at = f["modifiedTime"].as_str().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp())
                });

                all_files.push(crate::provider::RemoteFile { 
                    id, 
                    name, 
                    is_dir,
                    size,
                    modified_at,
                    hash,
                });
            }

            if let Some(next_token) = body["nextPageToken"].as_str() {
                page_token = Some(next_token.to_string());
            } else {
                break;
            }
        }

        Ok(all_files)
    }

    async fn get_metadata(&self, _cloud_path: &str) -> CloudResult<FileMetadata> {
        Ok(FileMetadata {
            hash: Some("dummy".to_string()),
            size: Some(0),
            modified_at: Some(0),
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

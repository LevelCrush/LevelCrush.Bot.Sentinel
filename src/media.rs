use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct MediaCache {
    cache_dir: PathBuf,
}

impl MediaCache {
    pub fn new(cache_dir: impl AsRef<Path>) -> Self {
        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
        }
    }

    pub async fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).await?;

        // Create subdirectories for organization
        let subdirs = ["images", "videos", "audio", "documents", "other"];
        for subdir in subdirs {
            fs::create_dir_all(self.cache_dir.join(subdir)).await?;
        }

        Ok(())
    }

    pub async fn download_attachment(
        &self,
        url: &str,
        filename: &str,
        content_type: Option<&str>,
    ) -> Result<PathBuf> {
        // Determine subdirectory based on content type
        let subdir = match content_type {
            Some(ct) if ct.starts_with("image/") => "images",
            Some(ct) if ct.starts_with("video/") => "videos",
            Some(ct) if ct.starts_with("audio/") => "audio",
            Some(ct) if ct.contains("pdf") || ct.contains("document") => "documents",
            _ => "other",
        };

        // Generate unique filename to avoid collisions
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("bin");
        let unique_filename = format!(
            "{}_{}.{}",
            Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file"),
            Uuid::new_v4(),
            extension
        );

        let file_path = self.cache_dir.join(subdir).join(&unique_filename);

        // Download the file
        let response = reqwest::get(url).await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to download attachment: HTTP {}",
                response.status()
            ));
        }

        // Stream to file
        let mut file = fs::File::create(&file_path).await?;
        let bytes = response.bytes().await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        info!("Downloaded attachment {} to {:?}", filename, file_path);

        Ok(file_path)
    }

    pub async fn cleanup_old_files(&self, days: i64) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let mut deleted_count = 0;

        // Walk through all subdirectories
        let subdirs = ["images", "videos", "audio", "documents", "other"];
        for subdir in subdirs {
            let dir_path = self.cache_dir.join(subdir);

            if !dir_path.exists() {
                continue;
            }

            let mut entries = fs::read_dir(&dir_path).await?;

            while let Some(entry) = entries.next_entry().await? {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        let modified_time = chrono::DateTime::<chrono::Utc>::from(modified);

                        if modified_time < cutoff {
                            if let Err(e) = fs::remove_file(entry.path()).await {
                                error!(
                                    "Failed to delete old cached file {:?}: {}",
                                    entry.path(),
                                    e
                                );
                            } else {
                                deleted_count += 1;
                                info!("Deleted old cached file: {:?}", entry.path());
                            }
                        }
                    }
                }
            }
        }

        Ok(deleted_count)
    }

    pub fn get_relative_path(&self, full_path: &Path) -> Option<String> {
        full_path
            .strip_prefix(&self.cache_dir)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string())
    }
}

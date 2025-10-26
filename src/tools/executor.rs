use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::permissions::PermissionManager;

pub struct ToolExecutor {
    base_path: PathBuf,
    permission_manager: Option<Arc<PermissionManager>>,
}

impl ToolExecutor {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            permission_manager: None,
        }
    }

    pub fn with_permissions(mut self, permission_manager: Arc<PermissionManager>) -> Self {
        self.permission_manager = Some(permission_manager);
        self
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        }
    }

    pub async fn read_file(&self, path: &str) -> Result<String> {
        let full_path = self.resolve_path(path);
        debug!("Reading file: {:?}", full_path);

        fs::read_to_string(&full_path)
            .await
            .with_context(|| format!("Failed to read file: {:?}", full_path))
    }

    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let full_path = self.resolve_path(path);

        // Check permissions if manager is available
        if let Some(ref pm) = self.permission_manager {
            let preview = if content.len() > 200 {
                &content[..200]
            } else {
                content
            };

            if !pm.request_file_write(path, preview) {
                warn!("File write denied by user: {:?}", full_path);
                anyhow::bail!("File write permission denied by user");
            }
        }

        debug!("Writing file: {:?}", full_path);

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&full_path, content)
            .await
            .with_context(|| format!("Failed to write file: {:?}", full_path))
    }

    pub async fn list_files(&self, path: &str) -> Result<Vec<String>> {
        let full_path = self.resolve_path(path);
        debug!("Listing files in: {:?}", full_path);

        let mut files = Vec::new();
        let mut entries = fs::read_dir(&full_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                let file_type = if entry.file_type().await?.is_dir() {
                    "dir"
                } else {
                    "file"
                };
                files.push(format!("{} ({})", name, file_type));
            }
        }

        Ok(files)
    }

    pub async fn search_files(&self, base_path: &str, pattern: &str) -> Result<Vec<String>> {
        let full_path = self.resolve_path(base_path);
        debug!("Searching for pattern '{}' in: {:?}", pattern, full_path);

        let mut matches = Vec::new();

        for entry in WalkDir::new(&full_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()).await {
                    if content.contains(pattern) {
                        if let Some(path_str) = entry.path().to_str() {
                            matches.push(path_str.to_string());
                        }
                    }
                }
            }
        }

        Ok(matches)
    }

    pub async fn execute_shell(&self, command: &str, working_dir: &str) -> Result<String> {
        let full_working_dir = self.resolve_path(working_dir);

        // Check permissions if manager is available
        if let Some(ref pm) = self.permission_manager {
            if !pm.request_shell_execution(command) {
                warn!("Shell execution denied by user: {}", command);
                anyhow::bail!("Shell execution permission denied by user");
            }
        }

        info!(
            "Executing shell command: {} in {:?}",
            command, full_working_dir
        );

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", command])
                .current_dir(&full_working_dir)
                .output()
                .await?
        } else {
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&full_working_dir)
                .output()
                .await?
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if output.status.success() {
            stdout.to_string()
        } else {
            format!("Command failed:\nStdout: {}\nStderr: {}", stdout, stderr)
        };

        Ok(result)
    }

    #[allow(dead_code)]
    pub async fn file_exists(&self, path: &str) -> bool {
        let full_path = self.resolve_path(path);
        fs::metadata(&full_path).await.is_ok()
    }

    #[allow(dead_code)]
    pub async fn delete_file(&self, path: &str) -> Result<()> {
        let full_path = self.resolve_path(path);
        debug!("Deleting file: {:?}", full_path);

        fs::remove_file(&full_path)
            .await
            .with_context(|| format!("Failed to delete file: {:?}", full_path))
    }

    #[allow(dead_code)]
    pub async fn copy_file(&self, from: &str, to: &str) -> Result<()> {
        let from_path = self.resolve_path(from);
        let to_path = self.resolve_path(to);
        debug!("Copying file from {:?} to {:?}", from_path, to_path);

        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::copy(&from_path, &to_path).await.with_context(|| {
            format!("Failed to copy file from {:?} to {:?}", from_path, to_path)
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_write_and_read_file() {
        let dir = tempdir().unwrap();
        let executor = ToolExecutor::new(dir.path());

        let content = "Hello, world!";
        executor.write_file("test.txt", content).await.unwrap();

        let read_content = executor.read_file("test.txt").await.unwrap();
        assert_eq!(content, read_content);
    }

    #[tokio::test]
    async fn test_list_files() {
        let dir = tempdir().unwrap();
        let executor = ToolExecutor::new(dir.path());

        executor.write_file("file1.txt", "content1").await.unwrap();
        executor.write_file("file2.txt", "content2").await.unwrap();

        let files = executor.list_files(".").await.unwrap();
        assert_eq!(files.len(), 2);
    }
}

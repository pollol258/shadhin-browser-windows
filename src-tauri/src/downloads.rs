use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub total_bytes: Option<u64>,
    pub received_bytes: u64,
    pub status: String, // "downloading" | "completed" | "failed" | "cancelled"
    pub created_at: String,
}

pub struct DownloadManager {
    file_path: PathBuf,
    items: Arc<Mutex<Vec<DownloadItem>>>,
}

impl DownloadManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join("downloads.json");
        let items = if file_path.exists() {
            match fs::read_to_string(&file_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Self {
            file_path,
            items: Arc::new(Mutex::new(items)),
        }
    }

    pub fn register_download(&self, url: &str, destination: &Path) -> DownloadItem {
        let filename = destination
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "download".to_string());

        let item = DownloadItem {
            id: Uuid::new_v4().to_string(),
            url: url.to_string(),
            filename,
            save_path: destination.to_string_lossy().to_string(),
            total_bytes: None,
            received_bytes: 0,
            status: "downloading".to_string(),
            created_at: chrono::Local::now().to_rfc3339(),
        };

        let mut items = self.items.lock().unwrap();
        items.insert(0, item.clone());
        let _ = self.persist(&items);
        item
    }

    pub fn complete_download(&self, id: &str, success: bool) {
        let mut items = self.items.lock().unwrap();
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            item.status = if success {
                "completed".to_string()
            } else {
                "failed".to_string()
            };
        }
        let _ = self.persist(&items);
    }

    pub fn complete_download_by_url(&self, url: &str, success: bool) {
        let mut items = self.items.lock().unwrap();
        if let Some(item) = items.iter_mut().find(|i| i.url == url && i.status == "downloading") {
            item.status = if success {
                "completed".to_string()
            } else {
                "failed".to_string()
            };
        }
        let _ = self.persist(&items);
    }

    pub fn find_by_url(&self, url: &str) -> Option<DownloadItem> {
        let items = self.items.lock().unwrap();
        items.iter().find(|i| i.url == url).cloned()
    }

    pub fn get_all(&self) -> Vec<DownloadItem> {
        self.items.lock().unwrap().clone()
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        let mut items = self.items.lock().unwrap();
        items.clear();
        self.persist(&items)?;
        Ok(())
    }

    pub fn open_file(path_str: &str) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", path_str])
                .spawn()?;
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("xdg-open")
                .arg(path_str)
                .spawn()?;
        }
        Ok(())
    }

    pub fn show_in_folder(path_str: &str) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", path_str))
                .spawn()?;
        }
        #[cfg(not(windows))]
        {
            if let Some(parent) = Path::new(path_str).parent() {
                std::process::Command::new("xdg-open")
                    .arg(parent)
                    .spawn()?;
            }
        }
        Ok(())
    }

    fn persist(&self, items: &[DownloadItem]) -> anyhow::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(items)?;
        fs::write(&self.file_path, json)?;
        Ok(())
    }
}

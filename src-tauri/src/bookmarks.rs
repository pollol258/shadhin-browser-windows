use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkItem {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub created_at: i64,
}

pub struct BookmarkManager {
    file_path: PathBuf,
    bookmarks: Arc<Mutex<Vec<BookmarkItem>>>,
}

impl BookmarkManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join("bookmarks.json");
        let bookmarks = if file_path.exists() {
            match fs::read_to_string(&file_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Self {
            file_path,
            bookmarks: Arc::new(Mutex::new(bookmarks)),
        }
    }

    pub fn get_all(&self) -> Vec<BookmarkItem> {
        let b = self.bookmarks.lock().unwrap();
        b.clone()
    }

    pub fn add(&self, title: String, url: String) -> BookmarkItem {
        let mut b = self.bookmarks.lock().unwrap();
        // If exact URL already exists, update title
        if let Some(existing) = b.iter_mut().find(|item| item.url == url) {
            existing.title = title;
            let item = existing.clone();
            drop(b);
            self.save_to_disk();
            return item;
        }

        let now = chrono::Utc::now().timestamp_millis();
        let item = BookmarkItem {
            id: Uuid::new_v4().to_string(),
            title,
            url,
            created_at: now,
        };
        b.insert(0, item.clone());
        drop(b);
        self.save_to_disk();
        item
    }

    pub fn remove(&self, id: &str) -> bool {
        let mut b = self.bookmarks.lock().unwrap();
        let initial_len = b.len();
        b.retain(|item| item.id != id);
        let removed = b.len() < initial_len;
        drop(b);
        if removed {
            self.save_to_disk();
        }
        removed
    }

    fn save_to_disk(&self) {
        let b = self.bookmarks.lock().unwrap();
        if let Ok(json) = serde_json::to_string_pretty(&*b) {
            let _ = fs::write(&self.file_path, json);
        }
    }
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub search_engine: String,
    pub homepage: String,
    pub download_dir: Option<String>,
    pub ad_block_enabled: bool,
    pub block_third_party_cookies: bool,
    pub do_not_track: bool,
    pub clear_data_on_exit: bool,
    pub password_manager_enabled: bool,
    pub dark_mode: String,
    #[serde(default)]
    pub ask_download_location: bool,
    #[serde(default = "default_true")]
    pub auto_open_downloads_panel: bool,
    #[serde(default = "default_true")]
    pub show_download_notification: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let default_download = dirs::download_dir().map(|p| p.to_string_lossy().to_string());
        Self {
            search_engine: "duckduckgo".to_string(),
            homepage: "https://duckduckgo.com".to_string(),
            download_dir: default_download,
            ad_block_enabled: true,
            block_third_party_cookies: true,
            do_not_track: true,
            clear_data_on_exit: false,
            password_manager_enabled: true,
            dark_mode: "system".to_string(),
            ask_download_location: false,
            auto_open_downloads_panel: true,
            show_download_notification: true,
        }
    }
}

pub struct SettingsManager {
    settings_path: PathBuf,
    settings: Arc<Mutex<AppSettings>>,
}

impl SettingsManager {
    pub fn load_from_path(path: &Path) -> AppSettings {
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => AppSettings::default(),
            }
        } else {
            AppSettings::default()
        }
    }

    pub fn save_to_path(path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(settings)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn new(app_data_dir: PathBuf) -> Self {
        let settings_path = app_data_dir.join("settings.json");
        let settings = Self::load_from_path(&settings_path);

        Self {
            settings_path,
            settings: Arc::new(Mutex::new(settings)),
        }
    }

    pub fn get(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }

    pub fn update(&self, new_settings: AppSettings) -> anyhow::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        *settings = new_settings;
        self.persist(&settings)?;
        Ok(())
    }

    fn persist(&self, settings: &AppSettings) -> anyhow::Result<()> {
        Self::save_to_path(&self.settings_path, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_default_values() {
        let defaults = AppSettings::default();

        assert_eq!(defaults.search_engine, "duckduckgo");
        assert_eq!(defaults.homepage, "https://duckduckgo.com");
        assert_eq!(defaults.ad_block_enabled, true);
        assert_eq!(defaults.block_third_party_cookies, true);
        assert_eq!(defaults.do_not_track, true);
        assert_eq!(defaults.clear_data_on_exit, false);
        assert_eq!(defaults.password_manager_enabled, true);
        assert_eq!(defaults.dark_mode, "system");
        assert_eq!(defaults.ask_download_location, false);
        assert_eq!(defaults.auto_open_downloads_panel, true);
        assert_eq!(defaults.show_download_notification, true);

        let expected_download = dirs::download_dir().map(|p| p.to_string_lossy().to_string());
        assert_eq!(defaults.download_dir, expected_download);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("shadhin_settings_test_{}", uuid::Uuid::new_v4()));
        let settings_file = temp_dir.join("settings.json");

        let custom_settings = AppSettings {
            search_engine: "google".to_string(),
            homepage: "https://google.com".to_string(),
            download_dir: Some("C:\\CustomDownloads".to_string()),
            ad_block_enabled: false,
            block_third_party_cookies: false,
            do_not_track: false,
            clear_data_on_exit: true,
            password_manager_enabled: false,
            dark_mode: "dark".to_string(),
            ask_download_location: true,
            auto_open_downloads_panel: false,
            show_download_notification: false,
        };

        // Save settings via helper
        SettingsManager::save_to_path(&settings_file, &custom_settings)
            .expect("save_to_path must succeed");

        // Load settings back via helper
        let loaded = SettingsManager::load_from_path(&settings_file);
        assert_eq!(loaded, custom_settings);

        // Also test through SettingsManager manager API
        let mgr = SettingsManager::new(temp_dir.clone());
        assert_eq!(mgr.get(), custom_settings);

        let mut updated = mgr.get();
        updated.search_engine = "brave".to_string();
        updated.ad_block_enabled = true;
        mgr.update(updated.clone()).expect("update must succeed");
        assert_eq!(mgr.get(), updated);

        let loaded_after_update = SettingsManager::load_from_path(&settings_file);
        assert_eq!(loaded_after_update, updated);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_missing_file_fallback() {
        let non_existent = std::env::temp_dir().join(format!("non_existent_{}.json", uuid::Uuid::new_v4()));
        let loaded = SettingsManager::load_from_path(&non_existent);
        assert_eq!(loaded, AppSettings::default());
    }

    #[test]
    fn test_corrupted_json_fallback() {
        let temp_dir = std::env::temp_dir().join(format!("shadhin_corrupt_test_{}", uuid::Uuid::new_v4()));
        let settings_file = temp_dir.join("settings.json");
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        fs::write(&settings_file, b"{ invalid json content !!!").expect("write corrupt file");

        let loaded = SettingsManager::load_from_path(&settings_file);
        assert_eq!(loaded, AppSettings::default());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_partial_json_deserialization_defaults() {
        // Test partial JSON where new fields are omitted
        let partial_json = r#"{
            "search_engine": "brave",
            "homepage": "https://search.brave.com",
            "download_dir": null,
            "ad_block_enabled": true,
            "block_third_party_cookies": true,
            "do_not_track": true,
            "clear_data_on_exit": false,
            "password_manager_enabled": true,
            "dark_mode": "dark"
        }"#;

        let parsed: AppSettings = serde_json::from_str(partial_json)
            .expect("partial JSON must deserialize with defaults");

        assert_eq!(parsed.search_engine, "brave");
        assert_eq!(parsed.homepage, "https://search.brave.com");
        assert_eq!(parsed.dark_mode, "dark");
        // Omitted fields should receive their defaults via #[serde(default)]
        assert_eq!(parsed.ask_download_location, false);
        assert_eq!(parsed.auto_open_downloads_panel, true);
        assert_eq!(parsed.show_download_notification, true);
    }
}

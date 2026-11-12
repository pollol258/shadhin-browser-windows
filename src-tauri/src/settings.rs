use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        }
    }
}

pub struct SettingsManager {
    settings_path: PathBuf,
    settings: Arc<Mutex<AppSettings>>,
}

impl SettingsManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let settings_path = app_data_dir.join("settings.json");
        let settings = if settings_path.exists() {
            match fs::read_to_string(&settings_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => AppSettings::default(),
            }
        } else {
            AppSettings::default()
        };

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
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(settings)?;
        fs::write(&self.settings_path, json)?;
        Ok(())
    }
}

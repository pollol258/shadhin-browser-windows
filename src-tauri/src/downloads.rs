use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, Webview};

#[cfg(windows)]
use webview2_com::Microsoft::Web::WebView2::Win32::*;
#[cfg(windows)]
use windows_core::Interface;

use crate::settings::SettingsManager;

#[cfg(windows)]
#[derive(Clone)]
pub struct NativeDownloadOp(pub ICoreWebView2DownloadOperation);
#[cfg(windows)]
unsafe impl Send for NativeDownloadOp {}
#[cfg(windows)]
unsafe impl Sync for NativeDownloadOp {}

#[cfg(windows)]
impl NativeDownloadOp {
    pub fn pause(&self) {
        unsafe {
            let _ = self.0.Pause();
        }
    }

    pub fn resume(&self) {
        unsafe {
            let _ = self.0.Resume();
        }
    }

    pub fn cancel(&self) {
        unsafe {
            let _ = self.0.Cancel();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub source_domain: String,
    pub filename: String,
    pub save_path: String,
    pub total_bytes: Option<u64>,
    pub received_bytes: u64,
    // "in_progress" | "paused" | "completed" | "failed" | "cancelled" | "interrupted"
    pub state: String,
    // Alias for backward compatibility
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub error_reason: Option<String>,
    pub started_at: String,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgressPayload {
    pub id: String,
    pub filename: String,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub state: String,
}

#[derive(Debug)]
pub struct ProgressTracker {
    pub last_update: std::time::Instant,
    pub last_bytes: u64,
    pub smoothed_speed: f64,
    pub last_emit: std::time::Instant,
    pub filename: String,
}

impl ProgressTracker {
    pub fn new(initial_bytes: u64, filename: String) -> Self {
        let now = std::time::Instant::now();
        Self {
            last_update: now,
            last_bytes: initial_bytes,
            smoothed_speed: 0.0,
            last_emit: now,
            filename,
        }
    }

    pub fn update(&mut self, current_bytes: u64, total_bytes: Option<u64>) -> (u64, Option<u64>, bool) {
        let now = std::time::Instant::now();
        let elapsed_secs = self.last_update.elapsed().as_secs_f64();
        if elapsed_secs >= 0.1 {
            let bytes_diff = current_bytes.saturating_sub(self.last_bytes);
            let instant_speed = bytes_diff as f64 / elapsed_secs;
            if self.smoothed_speed <= 0.0 {
                self.smoothed_speed = instant_speed;
            } else {
                self.smoothed_speed = 0.3 * instant_speed + 0.7 * self.smoothed_speed;
            }
            self.last_update = now;
            self.last_bytes = current_bytes;
        }

        let speed = self.smoothed_speed.max(0.0) as u64;
        let eta = if speed > 1024 && total_bytes.unwrap_or(0) > current_bytes {
            let remaining = total_bytes.unwrap() - current_bytes;
            Some((remaining as f64 / self.smoothed_speed) as u64)
        } else {
            None
        };

        // Throttle updates to ~5 per second (every 200ms) to prevent IPC flooding
        let should_emit = self.last_emit.elapsed() >= std::time::Duration::from_millis(200);
        if should_emit {
            self.last_emit = now;
        }

        (speed, eta, should_emit)
    }
}

pub struct DownloadManager {
    file_path: PathBuf,
    items: Arc<Mutex<Vec<DownloadItem>>>,
    #[cfg(windows)]
    active_ops: Arc<Mutex<HashMap<String, NativeDownloadOp>>>,
    progress_trackers: Arc<Mutex<HashMap<String, ProgressTracker>>>,
}

impl DownloadManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join("downloads.json");
        let mut items: Vec<DownloadItem> = if file_path.exists() {
            match fs::read_to_string(&file_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        // Normalize loaded items
        for item in &mut items {
            if item.state.is_empty() && !item.status.is_empty() {
                item.state = item.status.clone();
            }
            if item.status.is_empty() && !item.state.is_empty() {
                item.status = item.state.clone();
            }

            // On startup, any item left in_progress or paused becomes "interrupted"
            if item.state == "in_progress" || item.state == "paused" || item.state == "downloading" {
                item.state = "interrupted".to_string();
                item.status = "interrupted".to_string();
                if item.error_reason.is_none() {
                    item.error_reason = Some("Interrupted when browser closed".to_string());
                }
            }
        }

        Self {
            file_path,
            items: Arc::new(Mutex::new(items)),
            #[cfg(windows)]
            active_ops: Arc::new(Mutex::new(HashMap::new())),
            progress_trackers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(windows)]
    #[allow(clippy::too_many_arguments)]
    pub fn register_download(
        &self,
        id: String,
        url: &str,
        source_domain: &str,
        destination: &Path,
        total_bytes: Option<u64>,
        mime_type: Option<&str>,
        op: NativeDownloadOp,
    ) -> DownloadItem {
        let filename = destination
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "download".to_string());

        let now_str = chrono::Local::now().to_rfc3339();
        let item = DownloadItem {
            id: id.clone(),
            url: url.to_string(),
            source_domain: source_domain.to_string(),
            filename: filename.clone(),
            save_path: destination.to_string_lossy().to_string(),
            total_bytes,
            received_bytes: 0,
            state: "in_progress".to_string(),
            status: "in_progress".to_string(),
            error_reason: None,
            started_at: now_str,
            finished_at: None,
            mime_type: mime_type.map(|m| m.to_string()),
        };

        {
            let mut ops = self.active_ops.lock().unwrap();
            ops.insert(id.clone(), op);
        }
        {
            let mut trackers = self.progress_trackers.lock().unwrap();
            trackers.insert(id, ProgressTracker::new(0, filename));
        }

        let snapshot = {
            let mut items = self.items.lock().unwrap();
            items.insert(0, item.clone());
            items.clone()
        };

        // Non-blocking background persistence
        Self::persist_async(self.file_path.clone(), snapshot);

        item
    }

    pub fn notify_start(&self, app: &AppHandle, item: &DownloadItem, settings_mgr: &SettingsManager) {
        let app_h = app.clone();
        let item_c = item.clone();
        let auto_open = settings_mgr.get().auto_open_downloads_panel;

        // Dispatch asynchronously so tb.eval and IPC never block the active STA COM stack
        tauri::async_runtime::spawn(async move {
            let _ = app_h.emit("download-started", &item_c);
            if let Some(tb) = app_h.get_webview("toolbar") {
                let item_json = serde_json::to_string(&item_c).unwrap_or_default();
                let _ = tb.eval(format!(
                    "if (window.__onDownloadStarted) {{ window.__onDownloadStarted({}, {}); }}",
                    item_json, auto_open
                ));
            }
        });
    }

    pub fn on_progress(
        &self,
        app: &AppHandle,
        id: &str,
        received_bytes: u64,
        total_bytes: Option<u64>,
    ) {
        let (speed, eta, filename, should_emit) = {
            let mut trackers = self.progress_trackers.lock().unwrap();
            if let Some(tracker) = trackers.get_mut(id) {
                let (s, e, emit) = tracker.update(received_bytes, total_bytes);
                (s, e, tracker.filename.clone(), emit)
            } else {
                (0, None, "download.bin".to_string(), true)
            }
        };

        // Fast exit when throttled: avoid touching items lock and skip IPC dispatch
        if !should_emit {
            return;
        }

        let mut current_state = "in_progress".to_string();
        {
            let mut items = self.items.lock().unwrap();
            if let Some(item) = items.iter_mut().find(|i| i.id == id) {
                item.received_bytes = received_bytes;
                if total_bytes.is_some() {
                    item.total_bytes = total_bytes;
                }
                current_state = item.state.clone();
            }
        }

        let payload = DownloadProgressPayload {
            id: id.to_string(),
            filename,
            received_bytes,
            total_bytes,
            speed_bps: speed,
            eta_seconds: eta,
            state: current_state,
        };

        let app_h = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = app_h.emit("download-progress", &payload);
            if let Some(tb) = app_h.get_webview("toolbar") {
                let payload_json = serde_json::to_string(&payload).unwrap_or_default();
                let _ = tb.eval(format!(
                    "if (window.__onDownloadProgress) {{ window.__onDownloadProgress({}); }}",
                    payload_json
                ));
            }
        });
    }

    pub fn on_state_changed(
        &self,
        app: &AppHandle,
        id: &str,
        new_state: &str,
        error_reason: Option<String>,
        settings_mgr: &SettingsManager,
    ) {
        let finished = matches!(new_state, "completed" | "failed" | "cancelled" | "interrupted");
        let now_str = chrono::Local::now().to_rfc3339();

        let (updated_item, snapshot) = {
            let mut items = self.items.lock().unwrap();
            let opt = if let Some(item) = items.iter_mut().find(|i| i.id == id) {
                item.state = new_state.to_string();
                item.status = new_state.to_string();
                if let Some(err) = error_reason.clone() {
                    item.error_reason = Some(err);
                }
                if finished {
                    item.finished_at = Some(now_str);
                    if new_state == "completed" && item.total_bytes.is_none() {
                        item.total_bytes = Some(item.received_bytes);
                    }
                }
                Some(item.clone())
            } else {
                None
            };
            (opt, items.clone())
        };

        if finished {
            #[cfg(windows)]
            {
                let mut ops = self.active_ops.lock().unwrap();
                ops.remove(id);
            }
            let mut trackers = self.progress_trackers.lock().unwrap();
            trackers.remove(id);
        }

        // Non-blocking async persistence in background threadpool
        Self::persist_async(self.file_path.clone(), snapshot);

        if let Some(item) = updated_item {
            let app_h = app.clone();
            let settings = settings_mgr.get();
            let auto_open = settings.auto_open_downloads_panel;
            let show_notif = settings.show_download_notification;

            tauri::async_runtime::spawn(async move {
                let _ = app_h.emit("download-state-changed", &item);
                if finished {
                    let _ = app_h.emit("download-finished", &item);
                }
                if let Some(tb) = app_h.get_webview("toolbar") {
                    let item_json = serde_json::to_string(&item).unwrap_or_default();
                    let _ = tb.eval(format!(
                        "if (window.__onDownloadStateChanged) {{ window.__onDownloadStateChanged({}, {}, {}); }}",
                        item_json, auto_open, show_notif
                    ));
                }
            });
        }
    }

    pub fn get(&self, id: &str) -> Option<DownloadItem> {
        let items = self.items.lock().unwrap();
        items.iter().find(|i| i.id == id).cloned()
    }

    pub fn pause(&self, app: &AppHandle, id: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let op = {
                let ops = self.active_ops.lock().unwrap();
                ops.get(id).cloned()
            };
            if let Some(op) = op {
                app.run_on_main_thread(move || {
                    op.pause();
                })
                .map_err(|e| e.to_string())?;
                Ok(())
            } else {
                Err("Download is not actively running".to_string())
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (app, id);
            Err("Not supported on this platform".to_string())
        }
    }

    pub fn resume(&self, app: &AppHandle, id: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let op = {
                let ops = self.active_ops.lock().unwrap();
                ops.get(id).cloned()
            };
            if let Some(op) = op {
                app.run_on_main_thread(move || {
                    op.resume();
                })
                .map_err(|e| e.to_string())?;
                Ok(())
            } else {
                Err("Download is not actively paused".to_string())
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (app, id);
            Err("Not supported on this platform".to_string())
        }
    }

    pub fn cancel(&self, app: &AppHandle, id: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let op = {
                let ops = self.active_ops.lock().unwrap();
                ops.get(id).cloned()
            };
            if let Some(op) = op {
                app.run_on_main_thread(move || {
                    op.cancel();
                })
                .map_err(|e| e.to_string())?;
                Ok(())
            } else {
                Err("Download is not actively running".to_string())
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (app, id);
            Err("Not supported on this platform".to_string())
        }
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let snapshot = {
            let mut items = self.items.lock().unwrap();
            items.retain(|i| i.id != id);
            items.clone()
        };
        Self::persist_async(self.file_path.clone(), snapshot);
        Ok(())
    }

    pub fn delete_file(&self, id: &str) -> Result<(), String> {
        let (save_path_opt, snapshot) = {
            let mut items = self.items.lock().unwrap();
            let path = if let Some(pos) = items.iter().position(|i| i.id == id) {
                let item = items.remove(pos);
                Some(item.save_path)
            } else {
                None
            };
            (path, items.clone())
        };
        Self::persist_async(self.file_path.clone(), snapshot);

        if let Some(path_str) = save_path_opt {
            tauri::async_runtime::spawn_blocking(move || {
                let p = Path::new(&path_str);
                if p.exists() {
                    let _ = fs::remove_file(p);
                }
            });
        }
        Ok(())
    }

    pub fn file_exists(&self, id: &str) -> bool {
        let items = self.items.lock().unwrap();
        if let Some(item) = items.iter().find(|i| i.id == id) {
            Path::new(&item.save_path).exists()
        } else {
            false
        }
    }

    pub fn get_item(&self, id: &str) -> Option<DownloadItem> {
        let items = self.items.lock().unwrap();
        items.iter().find(|i| i.id == id).cloned()
    }

    pub fn get_all(&self) -> Vec<DownloadItem> {
        self.items.lock().unwrap().clone()
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        let snapshot = {
            let mut items = self.items.lock().unwrap();
            items.retain(|i| i.state == "in_progress" || i.state == "paused");
            items.clone()
        };
        Self::persist_async(self.file_path.clone(), snapshot);
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
            let clean_path = path_str.trim_start_matches(r"\\?\").replace('/', "\\");
            let path = Path::new(&clean_path);
            if path.exists() {
                std::process::Command::new("explorer.exe")
                    .arg(format!("/select,{}", clean_path))
                    .spawn()?;
            } else if let Some(parent) = path.parent() {
                Self::open_folder(parent)?;
            } else {
                Self::open_folder(path)?;
            }
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

    pub fn open_folder(folder_path: &Path) -> anyhow::Result<()> {
        let _ = fs::create_dir_all(folder_path);
        #[cfg(windows)]
        {
            let path_buf = folder_path.canonicalize().unwrap_or_else(|_| folder_path.to_path_buf());
            let path_str = path_buf.to_string_lossy();
            let clean_path = path_str.trim_start_matches(r"\\?\").replace('/', "\\");
            std::process::Command::new("explorer.exe")
                .arg(&clean_path)
                .spawn()?;
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("xdg-open")
                .arg(folder_path)
                .spawn()?;
        }
        Ok(())
    }

    fn persist_async(file_path: PathBuf, items: Vec<DownloadItem>) {
        tauri::async_runtime::spawn_blocking(move || {
            let _ = (|| -> anyhow::Result<()> {
                if let Some(parent) = file_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let capped = if items.len() > 500 {
                    &items[..500]
                } else {
                    &items[..]
                };
                let json = serde_json::to_string_pretty(capped)?;
                let tmp_path = file_path.with_extension("tmp");
                fs::write(&tmp_path, json)?;
                fs::rename(&tmp_path, &file_path)?;
                Ok(())
            })();
        });
    }
}

// ==========================================
// Safe Filename Resolution & Collision
// ==========================================

pub fn sanitize_filename(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 32 => '_',
            other => other,
        })
        .collect();

    // Strip trailing dots and spaces
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }

    if s.is_empty() {
        s = "download.bin".to_string();
    }

    // Check Windows reserved names
    let base_name = s.split('.').next().unwrap_or("").to_uppercase();

    let reserved = [
        "CON", "PRN", "AUX", "NUL",
        "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    if reserved.contains(&base_name.as_str()) {
        s = format!("_{}", s);
    }

    // Limit length (max 220 characters, preserving extension)
    if s.len() > 220 {
        let ext = Path::new(&s)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
            .unwrap_or_default();
        let max_stem_len = 220usize.saturating_sub(ext.len());
        let stem_part: String = Path::new(&s)
            .file_stem()
            .and_then(|st| st.to_str())
            .unwrap_or("download")
            .chars()
            .take(max_stem_len)
            .collect();
        s = format!("{}{}", stem_part, ext);
    }

    s
}

pub fn resolve_unique_filepath(target_dir: &Path, raw_filename: &str) -> PathBuf {
    let clean_name = sanitize_filename(raw_filename);
    let initial_path = target_dir.join(&clean_name);
    if !initial_path.exists() {
        return initial_path;
    }

    let stem = initial_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = initial_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let candidate = target_dir.join(format!("{} ({}){}", stem, counter, ext));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

// ==========================================
// Native Windows File Dialogs
// ==========================================

#[cfg(windows)]
pub fn pick_folder() -> Option<PathBuf> {
    std::thread::spawn(|| {
        unsafe {
            let co_init = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );

            let res = (|| -> Option<PathBuf> {
                let dialog: windows::Win32::UI::Shell::IFileOpenDialog =
                    windows::Win32::System::Com::CoCreateInstance(
                        &windows::Win32::UI::Shell::FileOpenDialog,
                        None,
                        windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
                    )
                    .ok()?;

                let _ = dialog.SetOptions(
                    windows::Win32::UI::Shell::FOS_PICKFOLDERS
                        | windows::Win32::UI::Shell::FOS_FORCEFILESYSTEM
                        | windows::Win32::UI::Shell::FOS_PATHMUSTEXIST,
                );

                let owner_hwnd = windows::Win32::Foundation::HWND::default();
                if dialog.Show(owner_hwnd).is_ok() {
                    let item = dialog.GetResult().ok()?;
                    let psz = item
                        .GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
                        .ok()?;
                    let path_str = psz.to_string().ok()?;
                    windows::Win32::System::Com::CoTaskMemFree(Some(psz.0 as *const _));
                    if !path_str.is_empty() {
                        return Some(PathBuf::from(path_str));
                    }
                }
                None
            })();

            if co_init.is_ok() {
                windows::Win32::System::Com::CoUninitialize();
            }

            res
        }
    })
    .join()
    .ok()
    .flatten()
}

#[cfg(not(windows))]
pub fn pick_folder() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
pub fn pick_save_file(default_dir: &Path, default_filename: &str) -> Option<PathBuf> {
    let default_dir = default_dir.to_path_buf();
    let default_filename = default_filename.to_string();
    std::thread::spawn(move || {
        unsafe {
            let co_init = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );

            let res = (|| -> Option<PathBuf> {
                let dialog: windows::Win32::UI::Shell::IFileSaveDialog =
                    windows::Win32::System::Com::CoCreateInstance(
                        &windows::Win32::UI::Shell::FileSaveDialog,
                        None,
                        windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
                    )
                    .ok()?;

                let name_clean = sanitize_filename(&default_filename);
                let name_h = windows::core::HSTRING::from(name_clean.as_str());
                let _ = dialog.SetFileName(&name_h);

                let dir_str = default_dir.to_string_lossy();
                let dir_h = windows::core::HSTRING::from(dir_str.as_ref());
                if let Ok(dir_item) = windows::Win32::UI::Shell::SHCreateItemFromParsingName::<
                    windows::core::PCWSTR,
                    _,
                    windows::Win32::UI::Shell::IShellItem,
                >(
                    windows::core::PCWSTR(dir_h.as_ptr()),
                    None,
                ) {
                    let _ = dialog.SetFolder(&dir_item);
                }

                // Never attach to active foreground window synchronously to prevent message pump deadlock
                let owner_hwnd = windows::Win32::Foundation::HWND::default();
                if dialog.Show(owner_hwnd).is_ok() {
                    let item = dialog.GetResult().ok()?;
                    let psz = item
                        .GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
                        .ok()?;
                    let path_str = psz.to_string().ok()?;
                    windows::Win32::System::Com::CoTaskMemFree(Some(psz.0 as *const _));
                    if !path_str.is_empty() {
                        return Some(PathBuf::from(path_str));
                    }
                }
                None
            })();

            if co_init.is_ok() {
                windows::Win32::System::Com::CoUninitialize();
            }

            res
        }
    })
    .join()
    .ok()
    .flatten()
}

#[cfg(not(windows))]
pub fn pick_save_file(_default_dir: &Path, _default_filename: &str) -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn format_interrupt_reason(reason: COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON) -> String {
    match reason {
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE => "None".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_FAILED => "File failed".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_ACCESS_DENIED => "Access denied".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_NO_SPACE => "Disk full".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_NAME_TOO_LONG => "Filename too long".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TOO_LARGE => "File too large".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_MALICIOUS => "File blocked (malicious)".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TRANSIENT_ERROR => "Transient error".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_BLOCKED_BY_POLICY => "Blocked by policy".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_SECURITY_CHECK_FAILED => "Security check failed".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TOO_SHORT => "File too short".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_HASH_MISMATCH => "Hash mismatch".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_FAILED => "Network failed".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_TIMEOUT => "Network timeout".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_DISCONNECTED => "Network disconnected".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_SERVER_DOWN => "Server down".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_INVALID_REQUEST => "Invalid request".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_FAILED => "Server error".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_NO_RANGE => "Range not supported".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_BAD_CONTENT => "Bad content".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_UNAUTHORIZED => "Unauthorized".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CERTIFICATE_PROBLEM => "Certificate problem".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_FORBIDDEN => "Forbidden".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_UNEXPECTED_RESPONSE => "Unexpected response".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED => "User Cancelled".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_SHUTDOWN => "App closed".to_string(),
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED => "User paused".to_string(),
        _ => "Unknown error".to_string(),
    }
}

// ==========================================
// Native Tab Download Hook (WebView2 COM)
// ==========================================

#[cfg(windows)]
pub fn hook_tab_download(
    app: AppHandle,
    webview: &Webview,
    download_mgr: Arc<DownloadManager>,
    settings_mgr: Arc<SettingsManager>,
) -> anyhow::Result<()> {
    webview.with_webview(move |platform_webview| {
        unsafe {
            let controller = platform_webview.controller();
            if let Ok(core) = controller.CoreWebView2() {
                if let Ok(core4) = core.cast::<ICoreWebView2_4>() {
                    let mut token = Default::default();
                    let dl_mgr = download_mgr.clone();
                    let st_mgr = settings_mgr.clone();
                    let app_h = app.clone();

                    let handler = webview2_com::DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
                        let Some(args) = args else { return Ok(()); };
                        let operation = args.DownloadOperation()?;

                        let download_id = uuid::Uuid::new_v4().to_string();

                        let url_str = {
                            let mut uri = windows_core::PWSTR(std::ptr::null_mut());
                            operation.Uri(&mut uri)?;
                            webview2_com::take_pwstr(uri)
                        };

                        let mime_type = {
                            let mut mime = windows_core::PWSTR(std::ptr::null_mut());
                            if operation.MimeType(&mut mime).is_ok() && !mime.0.is_null() {
                                let m = webview2_com::take_pwstr(mime);
                                if !m.is_empty() { Some(m) } else { None }
                            } else {
                                None
                            }
                        };

                        let total_bytes = {
                            let mut total: i64 = 0;
                            if operation.TotalBytesToReceive(&mut total).is_ok() && total > 0 {
                                Some(total as u64)
                            } else {
                                None
                            }
                        };

                        let proposed_path_str = {
                            let mut path_pwstr = windows_core::PWSTR(std::ptr::null_mut());
                            args.ResultFilePath(&mut path_pwstr)?;
                            webview2_com::take_pwstr(path_pwstr)
                        };
                        let proposed_path = PathBuf::from(&proposed_path_str);

                        let parsed_url = url::Url::parse(&url_str).ok();
                        let source_domain = parsed_url.as_ref()
                            .and_then(|u| u.host_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();

                        let raw_filename = proposed_path
                            .file_name()
                            .and_then(|f| f.to_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                parsed_url.as_ref()
                                    .and_then(|u| u.path_segments())
                                    .and_then(|mut segs| segs.next_back())
                                    .filter(|s| !s.is_empty() && s.contains('.'))
                                    .map(|s| s.to_string())
                            })
                            .or_else(|| {
                                parsed_url.as_ref()
                                    .and_then(|u| u.path_segments())
                                    .and_then(|mut segs| segs.next_back())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string())
                            })
                            .unwrap_or_else(|| "download.bin".to_string());

                        let settings = st_mgr.get();
                        let default_dir = settings
                            .download_dir
                            .as_ref()
                            .map(PathBuf::from)
                            .or_else(dirs::download_dir)
                            .unwrap_or_else(|| PathBuf::from("."));

                        // Option A: Auto-save immediately to downloads directory with unique filename
                        // Zero UI hang: never block with synchronous dialog or .join() on the STA UI thread
                        let final_dest: PathBuf = if settings.ask_download_location {
                            match pick_save_file(&default_dir, &raw_filename) {
                                Some(chosen_path) => chosen_path,
                                None => {
                                    let _ = args.SetCancel(true);
                                    let _ = args.SetHandled(true);
                                    return Ok(());
                                }
                            }
                        } else {
                            resolve_unique_filepath(&default_dir, &raw_filename)
                        };

                        let dest_hstring = windows_core::HSTRING::from(final_dest.to_string_lossy().as_ref());
                        args.SetResultFilePath(&dest_hstring)?;
                        args.SetHandled(true)?;

                        let item = dl_mgr.register_download(
                            download_id.clone(),
                            &url_str,
                            &source_domain,
                            &final_dest,
                            total_bytes,
                            mime_type.as_deref(),
                            NativeDownloadOp(operation.clone()),
                        );

                        // Notify toolbar of start (dispatched asynchronously)
                        dl_mgr.notify_start(&app_h, &item, &st_mgr);

                        // Hook progress
                        let op_bytes = operation.clone();
                        let dl_mgr_bytes = dl_mgr.clone();
                        let app_bytes = app_h.clone();
                        let id_bytes = download_id.clone();
                        let mut token_bytes = Default::default();

                        let bytes_handler = webview2_com::BytesReceivedChangedEventHandler::create(Box::new(move |_, _| {
                            let mut recv: i64 = 0;
                            if op_bytes.BytesReceived(&mut recv).is_ok() {
                                let recv_u64 = recv.max(0) as u64;
                                let mut total: i64 = 0;
                                let total_opt = if op_bytes.TotalBytesToReceive(&mut total).is_ok() && total > 0 {
                                    Some(total as u64)
                                } else {
                                    None
                                };
                                dl_mgr_bytes.on_progress(&app_bytes, &id_bytes, recv_u64, total_opt);
                            }
                            Ok(())
                        }));
                        let _ = operation.add_BytesReceivedChanged(&bytes_handler, &mut token_bytes);

                        // Hook state
                        let op_state = operation.clone();
                        let dl_mgr_state = dl_mgr.clone();
                        let app_state = app_h.clone();
                        let id_state = download_id.clone();
                        let st_mgr_state = st_mgr.clone();
                        let mut token_state = Default::default();

                        let state_handler = webview2_com::StateChangedEventHandler::create(Box::new(move |_, _| {
                            let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
                            if op_state.State(&mut state).is_ok() {
                                match state {
                                    COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS => {
                                        dl_mgr_state.on_state_changed(&app_state, &id_state, "in_progress", None, &st_mgr_state);
                                    }
                                    COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED => {
                                        dl_mgr_state.on_state_changed(&app_state, &id_state, "completed", None, &st_mgr_state);
                                    }
                                    COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED => {
                                        let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON::default();
                                        let reason_str = if op_state.InterruptReason(&mut reason).is_ok() {
                                            format_interrupt_reason(reason)
                                        } else {
                                            "Interrupted".to_string()
                                        };
                                        let status = match reason {
                                            COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED => "paused",
                                            COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED => "cancelled",
                                            _ => "interrupted",
                                        };
                                        dl_mgr_state.on_state_changed(&app_state, &id_state, status, Some(reason_str), &st_mgr_state);
                                    }
                                    _ => {}
                                }
                            }
                            Ok(())
                        }));
                        let _ = operation.add_StateChanged(&state_handler, &mut token_state);

                        Ok(())
                    }));

                    let _ = core4.add_DownloadStarting(&handler, &mut token);
                }
            }
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        // Forbidden characters replaced with '_'
        assert_eq!(sanitize_filename("file/with\\bad:chars*?\"<>|name.pdf"), "file_with_bad_chars______name.pdf");

        // Trailing dots and spaces stripped
        assert_eq!(sanitize_filename("document.docx...   "), "document.docx");

        // Empty filename fallback
        assert_eq!(sanitize_filename(""), "download.bin");
        assert_eq!(sanitize_filename("..."), "download.bin");

        // Reserved Windows names prepended with '_'
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_filename("prn"), "_prn");
        assert_eq!(sanitize_filename("aux.json"), "_aux.json");
        assert_eq!(sanitize_filename("nul"), "_nul");
        assert_eq!(sanitize_filename("com1.tar.gz"), "_com1.tar.gz");
        assert_eq!(sanitize_filename("lpt1"), "_lpt1");
    }

    #[test]
    fn test_resolve_unique_filepath_collision() {
        let temp_dir = std::env::temp_dir().join(format!("shadhin_dl_test_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);

        let initial = resolve_unique_filepath(&temp_dir, "report.pdf");
        assert_eq!(initial, temp_dir.join("report.pdf"));

        // Create the file so next call collides
        fs::write(&initial, b"content").expect("write test file");
        let second = resolve_unique_filepath(&temp_dir, "report.pdf");
        assert_eq!(second, temp_dir.join("report (1).pdf"));

        // Create the (1) file so next call increments to (2)
        fs::write(&second, b"content 2").expect("write test file 2");
        let third = resolve_unique_filepath(&temp_dir, "report.pdf");
        assert_eq!(third, temp_dir.join("report (2).pdf"));

        // Clean up test directory
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

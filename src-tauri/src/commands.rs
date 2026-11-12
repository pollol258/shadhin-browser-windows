use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewBuilder, WebviewUrl};

use crate::blocker::AdBlocker;
use crate::browser::navigation;
use crate::browser::tabs::{TabInfo, TabManager};
use crate::browser::{PAGE_INJECTION_SCRIPT, TOOLBAR_HEIGHT};
use crate::downloads::{DownloadItem, DownloadManager};
use crate::fullscreen::FullscreenManager;
use crate::passwords::{PasswordEntryView, PasswordManager};
use crate::settings::{AppSettings, SettingsManager};

pub struct AppState {
    pub tab_manager: Arc<Mutex<TabManager>>,
    pub settings_manager: Arc<SettingsManager>,
    pub password_manager: Arc<PasswordManager>,
    pub download_manager: Arc<DownloadManager>,
    pub ad_blocker: Arc<AdBlocker>,
    pub fullscreen_manager: Arc<FullscreenManager>,
    pub app_data_dir: PathBuf,
}

#[tauri::command]
pub async fn create_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    url: Option<String>,
) -> Result<TabInfo, String> {
    let window = app.get_window("main").ok_or("Main window not found")?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let logical_width = size.width as f64 / scale_factor;
    let logical_height = size.height as f64 / scale_factor;
    let content_height = (logical_height - TOOLBAR_HEIGHT).max(10.0);

    // Hide previously active webview
    let prev_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    if let Some(lbl) = prev_label {
        if let Some(prev_wv) = app.get_webview(&lbl) {
            let _ = prev_wv.hide();
        }
    }

    let tab_id = uuid::Uuid::new_v4().to_string();
    let tab_label = format!("tab_{}", tab_id);
    let initial_raw = url.unwrap_or_else(|| state.settings_manager.get().homepage);
    let target_url = navigation::normalize_url(&initial_raw, &state.settings_manager.get().search_engine);

    let parsed_url: url::Url = target_url
        .parse()
        .unwrap_or_else(|_| "https://duckduckgo.com".parse().unwrap());

    let mut builder = WebviewBuilder::new(&tab_label, WebviewUrl::External(parsed_url))
        .initialization_script(PAGE_INJECTION_SCRIPT)
        .zoom_hotkeys_enabled(true)
        .additional_browser_args(
            "--autoplay-policy=no-user-gesture-required --enable-features=VaapiVideoDecoder,OverlayScrollbar --enable-gpu-rasterization"
        );

    // Profile data dir for persistent cookies and session
    let profile_dir = state.app_data_dir.join("profiles").join("default");
    builder = builder.data_directory(profile_dir);

    // Tracker & Ad Blocker and internal action interception
    let blocker = state.ad_blocker.clone();
    let app_action = app.clone();
    builder = builder.on_web_resource_request(move |req, res| {
        let uri_str = req.uri().to_string();
        if uri_str.starts_with("http://shadhin.action/") {
            *res.status_mut() = http::StatusCode::NO_CONTENT;
            let path = uri_str.strip_prefix("http://shadhin.action/").unwrap_or_default().to_string();
            let action = path.split('?').next().unwrap_or_default().to_string();
            let app_h = app_action.clone();
            handle_injected_action(&app_h, &action);
            return;
        }
        if blocker.is_blocked(&uri_str) {
            *res.status_mut() = http::StatusCode::FORBIDDEN;
        }
    });

    // Document title update
    let app_title = app.clone();
    let tab_id_title = tab_id.clone();
    let tab_mgr_title = state.tab_manager.clone();
    builder = builder.on_document_title_changed(move |_wv, title| {
        {
            let mut mgr = tab_mgr_title.lock().unwrap();
            if let Some(tab) = mgr.find_tab_mut(&tab_id_title) {
                tab.title = title.clone();
            }
        }
        let _ = app_title.emit("tab-title-changed", serde_json::json!({
            "id": tab_id_title,
            "title": title
        }));
    });

    // Page load status update
    let app_load = app.clone();
    let tab_id_load = tab_id.clone();
    let tab_mgr_load = state.tab_manager.clone();
    builder = builder.on_page_load(move |_wv, payload| {
        let is_loading = matches!(payload.event(), tauri::webview::PageLoadEvent::Started);
        let url_str = payload.url().to_string();
        {
            let mut mgr = tab_mgr_load.lock().unwrap();
            if let Some(tab) = mgr.find_tab_mut(&tab_id_load) {
                tab.is_loading = is_loading;
                if !url_str.is_empty() {
                    tab.url = url_str.clone();
                }
            }
        }
        let _ = app_load.emit("tab-load-status", serde_json::json!({
            "id": tab_id_load,
            "isLoading": is_loading,
            "url": url_str
        }));
    });

    // Download interception
    let app_dl = app.clone();
    let dl_mgr = state.download_manager.clone();
    let settings_mgr = state.settings_manager.clone();
    builder = builder.on_download(move |_wv, event| match event {
        tauri::webview::DownloadEvent::Requested { url, destination } => {
            let settings = settings_mgr.get();
            if let Some(ref dir_str) = settings.download_dir {
                if let Some(fname) = destination.file_name() {
                    let custom_path = std::path::Path::new(dir_str).join(fname);
                    *destination = custom_path;
                }
            }
            let item = dl_mgr.register_download(url.as_str(), destination);
            let _ = app_dl.emit("download-started", &item);
            true
        }
        tauri::webview::DownloadEvent::Finished { url, success, .. } => {
            dl_mgr.complete_download_by_url(url.as_str(), success);
            let _ = app_dl.emit("download-finished", serde_json::json!({
                "url": url.as_str(),
                "success": success
            }));
            true
        }
        _ => true,
    });

    // Add child webview to main window
    let _child = window
        .add_child(
            builder,
            LogicalPosition::new(0.0, TOOLBAR_HEIGHT),
            LogicalSize::new(logical_width, content_height),
        )
        .map_err(|e| format!("Failed to create child webview: {}", e))?;

    let tab_info = TabInfo {
        id: tab_id.clone(),
        label: tab_label,
        title: "New Tab".to_string(),
        url: target_url,
        favicon: None,
        is_loading: true,
        can_go_back: false,
        can_go_forward: false,
    };

    {
        let mut mgr = state.tab_manager.lock().unwrap();
        mgr.add_tab(tab_info.clone());
    }

    let _ = app.emit("tab-created", &tab_info);
    let _ = app.emit("tab-switched", serde_json::json!({ "id": tab_id }));

    Ok(tab_info)
}

#[tauri::command]
pub async fn switch_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> Result<(), String> {
    let window = app.get_window("main").ok_or("Main window not found")?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let logical_width = size.width as f64 / scale_factor;
    let logical_height = size.height as f64 / scale_factor;
    let content_height = (logical_height - TOOLBAR_HEIGHT).max(10.0);

    let (active_label, all_tabs) = {
        let mut mgr = state.tab_manager.lock().unwrap();
        if !mgr.set_active(&tab_id) {
            return Err("Tab not found".to_string());
        }
        let active = mgr.get_active_tab().map(|t| t.label.clone());
        let tabs = mgr.tabs.clone();
        (active, tabs)
    };

    // Hide all other webviews, show target active webview
    for tab in all_tabs {
        if let Some(wv) = app.get_webview(&tab.label) {
            if Some(&tab.label) == active_label.as_ref() {
                let _ = wv.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
                let _ = wv.set_size(LogicalSize::new(logical_width, content_height));
                let _ = wv.show();
            } else {
                let _ = wv.hide();
            }
        }
    }

    let _ = app.emit("tab-switched", serde_json::json!({ "id": tab_id }));
    Ok(())
}

#[tauri::command]
pub async fn close_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> Result<(), String> {
    let (removed_tab, next_active_id, is_empty) = {
        let mut mgr = state.tab_manager.lock().unwrap();
        let removed = mgr.remove_tab(&tab_id);
        let next_active = mgr.active_tab_id.clone();
        let empty = mgr.tabs.is_empty();
        (removed, next_active, empty)
    };

    if let Some(tab) = removed_tab {
        if let Some(wv) = app.get_webview(&tab.label) {
            let _ = wv.close();
        }
        let _ = app.emit("tab-closed", serde_json::json!({ "id": tab_id }));
    }

    if is_empty {
        // If all tabs closed, create a fresh new tab
        let _ = create_tab(app, state, None).await?;
    } else if let Some(next_id) = next_active_id {
        switch_tab(app, state, next_id).await?;
    }

    Ok(())
}

#[tauri::command]
pub async fn close_active_tab(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let active_id = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.active_tab_id.clone()
    };

    if let Some(id) = active_id {
        close_tab(app, state, id).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn navigate_to(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };

    if let Some(lbl) = active_label {
        if let Some(wv) = app.get_webview(&lbl) {
            let settings = state.settings_manager.get();
            let target = navigation::normalize_url(&url, &settings.search_engine);
            if let Ok(parsed) = target.parse::<url::Url>() {
                wv.navigate(parsed).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn go_back(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    if let Some(lbl) = active_label {
        if let Some(wv) = app.get_webview(&lbl) {
            let _ = wv.eval("window.history.back()");
        }
    }
    Ok(())
}

#[tauri::command]
pub fn go_forward(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    if let Some(lbl) = active_label {
        if let Some(wv) = app.get_webview(&lbl) {
            let _ = wv.eval("window.history.forward()");
        }
    }
    Ok(())
}

#[tauri::command]
pub fn reload(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    if let Some(lbl) = active_label {
        if let Some(wv) = app.get_webview(&lbl) {
            let _ = wv.reload();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn stop(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    if let Some(lbl) = active_label {
        if let Some(wv) = app.get_webview(&lbl) {
            let _ = wv.eval("window.stop()");
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_tabs(state: State<'_, AppState>) -> Result<Vec<TabInfo>, String> {
    let mgr = state.tab_manager.lock().unwrap();
    Ok(mgr.tabs.clone())
}

#[tauri::command]
pub fn get_active_tab(state: State<'_, AppState>) -> Result<Option<TabInfo>, String> {
    let mgr = state.tab_manager.lock().unwrap();
    Ok(mgr.get_active_tab().cloned())
}

#[tauri::command]
pub fn window_minimize(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_window("main") {
        let _ = win.minimize();
    }
    Ok(())
}

#[tauri::command]
pub fn window_maximize(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_window("main") {
        if win.is_maximized().unwrap_or(false) {
            let _ = win.unmaximize();
        } else {
            let _ = win.maximize();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn window_close(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_window("main") {
        let _ = win.close();
    }
    Ok(())
}

#[tauri::command]
pub fn window_start_dragging(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_window("main") {
        let _ = win.start_dragging();
    }
    Ok(())
}

#[tauri::command]
pub fn get_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadItem>, String> {
    Ok(state.download_manager.get_all())
}

#[tauri::command]
pub fn open_download_file(path: String) -> Result<(), String> {
    DownloadManager::open_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_download_folder(path: String) -> Result<(), String> {
    DownloadManager::show_in_folder(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_downloads(state: State<'_, AppState>) -> Result<(), String> {
    state.download_manager.clear().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings_manager.get())
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    state.ad_blocker.set_enabled(settings.ad_block_enabled);
    state.settings_manager.update(settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_passwords(state: State<'_, AppState>) -> Result<Vec<PasswordEntryView>, String> {
    Ok(state.password_manager.get_all_entries())
}

#[tauri::command]
pub fn save_password(
    app: AppHandle,
    state: State<'_, AppState>,
    origin: String,
    username: String,
    password: String,
) -> Result<(), String> {
    state
        .password_manager
        .save_password(&origin, &username, &password)
        .map_err(|e| e.to_string())?;

    let _ = app.emit("password-saved", serde_json::json!({ "origin": origin, "username": username }));
    Ok(())
}

#[tauri::command]
pub fn delete_password(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.password_manager.delete_password(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_credentials_for_origin(
    state: State<'_, AppState>,
    origin: String,
) -> Result<Option<(String, String)>, String> {
    if !state.settings_manager.get().password_manager_enabled {
        return Ok(None);
    }
    Ok(state.password_manager.get_credentials_for_origin(&origin))
}

#[tauri::command]
pub fn clear_browsing_data(
    app: AppHandle,
    state: State<'_, AppState>,
    clear_cache: bool,
    clear_cookies: bool,
    clear_history: bool,
    clear_passwords: bool,
) -> Result<(), String> {
    if clear_passwords {
        let _ = state.password_manager.clear_all();
    }
    if clear_history {
        let _ = state.download_manager.clear();
    }

    // Clear localStorage and sessionStorage on open webviews
    let tabs = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.tabs.clone()
    };
    for tab in tabs {
        if let Some(wv) = app.get_webview(&tab.label) {
            let _ = wv.eval("try { localStorage.clear(); sessionStorage.clear(); } catch(e){}");
        }
    }

    let _ = (clear_cache, clear_cookies);
    Ok(())
}

#[tauri::command]
pub async fn new_private_window(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // In-memory private window mode
    let private_id = uuid::Uuid::new_v4().to_string();
    let win_label = format!("private_{}", private_id);

    let win = tauri::window::WindowBuilder::new(&app, &win_label)
        .title("Shadhin-Browser (Private Window)")
        .inner_size(1100.0, 750.0)
        .decorations(false)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    let private_profile_dir = state
        .app_data_dir
        .join("profiles")
        .join(format!("incognito_{}", private_id));

    let app_action = app.clone();
    let blocker = state.ad_blocker.clone();
    let wv_builder = WebviewBuilder::new(
        format!("pv_wv_{}", private_id),
        WebviewUrl::External("https://duckduckgo.com".parse().unwrap()),
    )
    .data_directory(private_profile_dir)
    .incognito(true)
    .zoom_hotkeys_enabled(true)
    .initialization_script(PAGE_INJECTION_SCRIPT)
    .on_web_resource_request(move |req, res| {
        let uri_str = req.uri().to_string();
        if uri_str.starts_with("http://shadhin.action/") {
            *res.status_mut() = http::StatusCode::NO_CONTENT;
            let path = uri_str.strip_prefix("http://shadhin.action/").unwrap_or_default().to_string();
            let action = path.split('?').next().unwrap_or_default().to_string();
            let app_h = app_action.clone();
            handle_injected_action(&app_h, &action);
            return;
        }
        if blocker.is_blocked(&uri_str) {
            *res.status_mut() = http::StatusCode::FORBIDDEN;
        }
    });

    win.add_child(
        wv_builder,
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(1100.0, 750.0),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn toggle_adblock(state: State<'_, AppState>) -> Result<bool, String> {
    let current = state.ad_blocker.is_enabled();
    let new_val = !current;
    state.ad_blocker.set_enabled(new_val);
    let mut settings = state.settings_manager.get();
    settings.ad_block_enabled = new_val;
    let _ = state.settings_manager.update(settings);
    Ok(new_val)
}

/// Set ad-blocker to a specific enabled state (called from tracker popover toggle button).
#[tauri::command]
pub fn toggle_ad_blocker(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    state.ad_blocker.set_enabled(enabled);
    let mut settings = state.settings_manager.get();
    settings.ad_block_enabled = enabled;
    let _ = state.settings_manager.update(settings);
    Ok(enabled)
}

#[tauri::command]
pub fn get_blocked_count(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.ad_blocker.get_blocked_count())
}

#[tauri::command]
pub fn on_login_submitted(
    app: AppHandle,
    origin: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let _ = app.emit("password-save-prompt", serde_json::json!({
        "origin": origin,
        "username": username,
        "password": password
    }));
    Ok(())
}

#[tauri::command]
pub fn on_fullscreen_element_changed(
    app: AppHandle,
    state: State<'_, AppState>,
    is_fullscreen: bool,
) -> Result<(), String> {
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    state
        .fullscreen_manager
        .set_fullscreen(&app, active_label.as_deref(), is_fullscreen)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn focus_address_bar(app: AppHandle) -> Result<(), String> {
    if let Some(tb) = app.get_webview("toolbar") {
        let _ = tb.set_focus();
    }
    let _ = app.emit("focus-address-bar", ());
    Ok(())
}

#[tauri::command]
pub fn toggle_devtools(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    if let Some(lbl) = active_label {
        if let Some(wv) = app.get_webview(&lbl) {
            let _ = wv.eval(
                "if (window.__TAURI_INTERNALS__) { console.log('DevTools requested'); }"
            );
        }
    }
    Ok(())
}

/// Central dispatcher for shortcuts and actions sent from injected content scripts
pub fn handle_injected_action(app: &AppHandle, action: &str) {
    let state = app.state::<AppState>();
    match action {
        "create_tab" => {
            let app_c = app.clone();
            tauri::async_runtime::spawn(async move {
                let st = app_c.state::<AppState>();
                let _ = create_tab(app_c.clone(), st, None).await;
            });
        }
        "close_active_tab" => {
            let app_c = app.clone();
            tauri::async_runtime::spawn(async move {
                let st = app_c.state::<AppState>();
                let _ = close_active_tab(app_c.clone(), st).await;
            });
        }
        "reload" => {
            let _ = reload(app.clone(), state);
        }
        "go_back" => {
            let _ = go_back(app.clone(), state);
        }
        "go_forward" => {
            let _ = go_forward(app.clone(), state);
        }
        "stop" => {
            let _ = stop(app.clone(), state);
        }
        "new_private_window" => {
            let _ = new_private_window(app.clone(), state);
        }
        "focus_address_bar" => {
            if let Some(tb) = app.get_webview("toolbar") {
                let _ = tb.set_focus();
            }
            let _ = app.emit("focus-address-bar", ());
        }
        "toggle_downloads" => {
            let _ = app.emit("trigger-shortcut", "toggle-downloads");
        }
        "open_settings" => {
            let _ = app.emit("trigger-shortcut", "open-settings");
        }
        "switch_next_tab" => {
            let _ = app.emit("trigger-shortcut", "switch-next-tab");
        }
        "switch_prev_tab" => {
            let _ = app.emit("trigger-shortcut", "switch-prev-tab");
        }
        "switch_first_tab" => {
            let _ = app.emit("trigger-shortcut", "switch-first-tab");
        }
        "switch_last_tab" => {
            let _ = app.emit("trigger-shortcut", "switch-last-tab");
        }
        other if other.starts_with("switch_tab_") => {
            let _ = app.emit("trigger-shortcut", other);
        }
        _ => {}
    }
}

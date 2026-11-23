use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use serde::Serialize;
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
    pub bookmark_manager: Arc<crate::bookmarks::BookmarkManager>,
    pub fullscreen_manager: Arc<FullscreenManager>,
    pub toolbar_expanded: Arc<AtomicBool>,
    pub last_create_tab: Arc<Mutex<Option<std::time::Instant>>>,
    pub app_data_dir: PathBuf,
}

#[cfg(windows)]
fn raise_toolbar_to_top(app: &AppHandle) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    if let Some(tb) = app.get_webview("toolbar") {
        let _ = tb.with_webview(|p| unsafe {
            let controller = p.controller();
            let mut hwnd: *mut core::ffi::c_void = std::ptr::null_mut();
            if controller.ParentWindow(&mut hwnd as *mut _ as *mut _).is_ok()
                && !hwnd.is_null()
            {
                let _ = SetWindowPos(
                    windows::Win32::Foundation::HWND(hwnd),
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        });
    }
}

#[cfg(not(windows))]
fn raise_toolbar_to_top(_app: &AppHandle) {}

#[tauri::command]
pub async fn create_tab(
    app: AppHandle,
    url: Option<String>,
) -> Result<TabInfo, String> {
    let state = app.state::<AppState>();

    {
        let mut g = state.last_create_tab.lock().unwrap();
        if let Some(t) = *g {
            if t.elapsed() < std::time::Duration::from_millis(120) {
                return Err("create_tab throttled".to_string());
            }
        }
        *g = Some(std::time::Instant::now());
    }

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

    // Check if new tab should load local newtab.html
    let is_local_newtab = match url.as_deref() {
        None | Some("newtab.html") | Some("about:newtab") | Some("shadhin://newtab") => true,
        Some(raw) if raw.trim() == "about:blank" || raw.trim().is_empty() => true,
        _ => false,
    };

    let (webview_url, target_url) = if is_local_newtab {
        (WebviewUrl::App("newtab.html".into()), "newtab.html".to_string())
    } else {
        let (search_engine, homepage) = {
            let s = state.settings_manager.get();
            (s.search_engine.clone(), s.homepage.clone())
        };
        let raw = url.as_deref().unwrap_or(&homepage);
        let normalized = navigation::normalize_url(raw, &search_engine);
        let parsed: url::Url = normalized
            .parse()
            .unwrap_or_else(|_| "https://duckduckgo.com".parse().unwrap());
        (WebviewUrl::External(parsed), normalized)
    };

    let tab_info = TabInfo {
        id: tab_id.clone(),
        label: tab_label.clone(),
        title: "New Tab".to_string(),
        url: target_url,
        favicon: None,
        is_loading: !is_local_newtab,
        can_go_back: false,
        can_go_forward: false,
    };

    {
        let mut mgr = state.tab_manager.lock().unwrap();
        mgr.add_tab(tab_info.clone());
    }

    let mut builder = WebviewBuilder::new(&tab_label, webview_url)
        .zoom_hotkeys_enabled(true)
        .initialization_script(PAGE_INJECTION_SCRIPT)
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
        if uri_str.starts_with("tauri://")
            || uri_str.starts_with("http://tauri.localhost")
            || uri_str.starts_with("https://tauri.localhost")
        {
            return;
        }
        if uri_str.starts_with("http://shadhin.action/") {
            *res.status_mut() = http::StatusCode::NO_CONTENT;
            let path = uri_str.strip_prefix("http://shadhin.action/").unwrap_or_default().to_string();
            let app_h = app_action.clone();
            handle_injected_action(&app_h, &path);
            return;
        }
        if blocker.is_blocked(&uri_str) {
            *res.status_mut() = http::StatusCode::FORBIDDEN;
            let count = blocker.get_blocked_count();
            let _ = app_action.emit("tracker-blocked", serde_json::json!({
                "count": count,
                "url": uri_str
            }));
            if let Some(tb) = app_action.get_webview("toolbar") {
                let _ = tb.eval(&format!(
                    "if (window.__onTrackerBlocked) {{ window.__onTrackerBlocked({}); }}",
                    count
                ));
            }
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

    // Option A: Non-blocking download handling for non-Windows platforms (Windows uses native COM hook)
    #[cfg(not(windows))]
    {
        let st_dl = state.settings_manager.clone();
        builder = builder.on_download(move |_wv, event| match event {
            tauri::webview::DownloadEvent::Requested { destination, .. } => {
                let settings = st_dl.get();
                let default_dir = settings
                    .download_dir
                    .as_ref()
                    .map(PathBuf::from)
                    .or_else(dirs::download_dir)
                    .unwrap_or_else(|| PathBuf::from("."));
                let raw = destination
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("download.bin");
                *destination = crate::downloads::resolve_unique_filepath(&default_dir, raw);
                true
            }
            _ => true,
        });
    }

    // Page load status update and deferred native hooks
    let app_load = app.clone();
    let tab_id_load = tab_id.clone();
    let tab_label_load = tab_label.clone();
    let tab_mgr_load = state.tab_manager.clone();
    let hooks_wired = Arc::new(AtomicBool::new(false));
    let state_dl_hooks = state.download_manager.clone();
    let state_st_hooks = state.settings_manager.clone();
    let state_bl_hooks = state.ad_blocker.clone();

    builder = builder.on_page_load(move |_wv, payload| {
        let is_loading = matches!(payload.event(), tauri::webview::PageLoadEvent::Started);
        let url_str = payload.url().to_string();

        if is_loading {
            if !hooks_wired.swap(true, Ordering::AcqRel) {
                let app_h = app_load.clone();
                let label = tab_label_load.clone();
                let dl = state_dl_hooks.clone();
                let st = state_st_hooks.clone();
                let bl = state_bl_hooks.clone();
                tauri::async_runtime::spawn(async move {
                    // give the WebView2 controller one tick to start pumping
                    tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                    #[cfg(windows)]
                    if let Some(wv) = app_h.get_webview(&label) {
                        let _ = crate::downloads::hook_tab_download(app_h.clone(), &wv, dl, st);
                        let _ = crate::blocker::hook_tab_blocker(app_h.clone(), &wv, bl);
                        let _ = crate::fullscreen::hook_tab_fullscreen(app_h, &wv);
                    }
                });
            }
        }

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

    // Intercept new window requests (target="_blank", window.open, "Open link in new window")
    let app_new_window = app.clone();
    builder = builder.on_new_window(move |url, _features| {
        let scheme = url.scheme();
        if scheme == "http" || scheme == "https" {
            let _ = app_new_window.emit("open-url-in-new-tab", url.to_string());
        }
        tauri::webview::NewWindowResponse::Deny
    });

    // Add child webview to main window
    let _child = window
        .add_child(
            builder,
            LogicalPosition::new(0.0, TOOLBAR_HEIGHT),
            LogicalSize::new(logical_width, content_height),
        )
        .map_err(|e| format!("Failed to create child webview: {}", e))?;

    raise_toolbar_to_top(&app);

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
    let is_fs = state.fullscreen_manager.is_fullscreen();
    for tab in all_tabs {
        if let Some(wv) = app.get_webview(&tab.label) {
            if Some(&tab.label) == active_label.as_ref() {
                if is_fs {
                    let _ = wv.set_position(LogicalPosition::new(0.0, 0.0));
                    let _ = wv.set_size(LogicalSize::new(logical_width, logical_height));
                } else {
                    let _ = wv.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
                    let _ = wv.set_size(LogicalSize::new(logical_width, content_height));
                }
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
        let _ = create_tab(app, None).await?;
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
pub fn pause_download(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.download_manager.pause(&app, &id)
}

#[tauri::command]
pub fn resume_download(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.download_manager.resume(&app, &id)
}

#[tauri::command]
pub fn cancel_download(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.download_manager.cancel(&app, &id)
}

#[tauri::command]
pub fn retry_download(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let item = state.download_manager.get(&id).ok_or_else(|| "Download not found".to_string())?;
    let tab_label = {
        let tm = state.tab_manager.lock().unwrap();
        tm.get_active_tab().map(|t| t.label.clone())
    };

    if let Some(label) = tab_label {
        if let Some(wv) = app.get_webview(&label) {
            let script = format!(
                "{{ const a = document.createElement('a'); a.href = {}; a.download = ''; document.body.appendChild(a); a.click(); a.remove(); }}",
                serde_json::to_string(&item.url).unwrap_or_default()
            );
            wv.eval(&script).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err("No active browser tab found to retry download".to_string())
}

#[tauri::command]
pub fn remove_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.download_manager.remove(&id)
}

#[tauri::command]
pub fn delete_download_file(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.download_manager.delete_file(&id)
}

#[tauri::command]
pub async fn pick_download_folder() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::downloads::pick_folder().map(|p| p.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_downloads_folder(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings_manager.get();
    let folder = settings
        .download_dir
        .as_ref()
        .map(PathBuf::from)
        .or_else(dirs::download_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    DownloadManager::open_folder(&folder).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_download_file_exists(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    Ok(state.download_manager.file_exists(&id))
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

fn spawn_browser_window(
    app: AppHandle,
    url: url::Url,
    private: bool,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let id = uuid::Uuid::new_v4().to_string();

    let win_label = if private {
        format!("private_{}", id)
    } else {
        format!("window_{}", id)
    };

    let profile_subdir = if private {
        format!("profiles/incognito_{}", id)
    } else {
        // Use a unique directory per window to prevent WebView2 lock conflicts
        // with the main window which uses "profiles/default".
        format!("profiles/window_{}", id)
    };
    let profile_dir = state.app_data_dir.join(profile_subdir);

    let app_action = app.clone();
    let blocker = state.ad_blocker.clone();

    // Use WebviewWindowBuilder instead of WindowBuilder + add_child
    let win_builder = tauri::WebviewWindowBuilder::new(
        &app,
        &win_label,
        tauri::WebviewUrl::External(url),
    )
    .title(if private { "Shadhin-Browser (Private)" } else { "Shadhin-Browser" })
    .inner_size(1100.0, 750.0)
    .decorations(true) // IMPORTANT: Allow native OS title bar so user can close/minimize
    .incognito(private)
    .data_directory(profile_dir)
    .zoom_hotkeys_enabled(true)
    .initialization_script(PAGE_INJECTION_SCRIPT)
    .on_web_resource_request(move |req, res| {
        let uri_str = req.uri().to_string();
        if uri_str.starts_with("http://shadhin.action/") {
            *res.status_mut() = http::StatusCode::NO_CONTENT;
            let path = uri_str
                .strip_prefix("http://shadhin.action/")
                .unwrap_or_default()
                .to_string();
            let app_h = app_action.clone();
            handle_injected_action(&app_h, &path);
            return;
        }
        if blocker.is_blocked(&uri_str) {
            *res.status_mut() = http::StatusCode::FORBIDDEN;
            let count = blocker.get_blocked_count();
            let _ = app_action.emit("tracker-blocked", serde_json::json!({
                "count": count,
                "url": uri_str
            }));
            if let Some(tb) = app_action.get_webview("toolbar") {
                let _ = tb.eval(&format!(
                    "if (window.__onTrackerBlocked) {{ window.__onTrackerBlocked({}); }}",
                    count
                ));
            }
        }
    });

    #[cfg(not(windows))]
    let win_builder = {
        let st_dl_win = state.settings_manager.clone();
        win_builder.on_download(move |_wv, event| match event {
            tauri::webview::DownloadEvent::Requested { destination, .. } => {
                let settings = st_dl_win.get();
                let default_dir = settings
                    .download_dir
                    .as_ref()
                    .map(PathBuf::from)
                    .or_else(dirs::download_dir)
                    .unwrap_or_else(|| PathBuf::from("."));
                let raw = destination
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("download.bin");
                *destination = crate::downloads::resolve_unique_filepath(&default_dir, raw);
                true
            }
            _ => true,
        })
    };

    let _win = win_builder
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn new_private_window(
    app: AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let url: url::Url = "https://duckduckgo.com".parse().unwrap();
    spawn_browser_window(app, url, true)
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
pub fn tracker_blocked_dom(
    app: AppHandle,
    state: State<'_, AppState>,
    count: Option<usize>,
) -> Result<usize, String> {
    if !state.ad_blocker.is_enabled() {
        return Ok(state.ad_blocker.get_blocked_count());
    }
    let c = count.unwrap_or(1);
    let total = state.ad_blocker.add_blocked(c);
    println!("[AdBlock] DOM Blocked via invoke: +{} (total: {})", c, total);
    let _ = app.emit("tracker-blocked", serde_json::json!({
        "count": total
    }));
    if let Some(tb) = app.get_webview("toolbar") {
        let _ = tb.eval(&format!(
            "if (window.__onTrackerBlocked) {{ window.__onTrackerBlocked({}); }}",
            total
        ));
    }
    Ok(total)
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
    _state: State<'_, AppState>,
    is_fullscreen: bool,
) -> Result<(), String> {
    crate::fullscreen::handle_fullscreen_transition(&app, is_fullscreen);
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

#[derive(Debug, Clone, Serialize)]
pub struct WindowSizeInfo {
    pub width: f64,
    pub height: f64,
}

#[tauri::command]
pub fn get_window_size(app: AppHandle) -> Result<WindowSizeInfo, String> {
    if let Some(win) = app.get_window("main") {
        let size = win.inner_size().map_err(|e| e.to_string())?;
        let scale = win.scale_factor().unwrap_or(1.0);
        Ok(WindowSizeInfo {
            width: size.width as f64 / scale,
            height: size.height as f64 / scale,
        })
    } else {
        Err("Main window not found".to_string())
    }
}

#[tauri::command]
pub fn log_frontend(_msg: String) {}

#[tauri::command]
pub fn set_toolbar_expanded(app: AppHandle, expanded: bool, height: Option<f64>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.fullscreen_manager.is_fullscreen() {
        return Ok(());
    }
    state.toolbar_expanded.store(expanded, Ordering::Relaxed);

    if let Some(win) = app.get_window("main") {
        let size = win.inner_size().map_err(|e| e.to_string())?;
        let scale = win.scale_factor().unwrap_or(1.0);
        let width = size.width as f64 / scale;
        let win_height = size.height as f64 / scale;

        let target_height = if expanded {
            height.unwrap_or(win_height).min(win_height)
        } else {
            TOOLBAR_HEIGHT
        };

        if let Some(tb) = app.get_webview("toolbar") {
            let _ = tb.set_size(LogicalSize::new(width, target_height));
            if expanded {
                let _ = tb.set_focus();

                #[cfg(windows)]
                {
                    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOMOVE, SWP_NOSIZE};
                    let _ = tb.with_webview(|platform_webview| {
                        unsafe {
                            let controller = platform_webview.controller();
                            let mut raw_hwnd: *mut core::ffi::c_void = std::ptr::null_mut();
                            if controller.ParentWindow(&mut raw_hwnd as *mut _ as *mut _).is_ok() && !raw_hwnd.is_null() {
                                let _ = SetWindowPos(
                                    windows::Win32::Foundation::HWND(raw_hwnd),
                                    HWND_TOP,
                                    0, 0, 0, 0,
                                    SWP_NOMOVE | SWP_NOSIZE,
                                );
                            }
                        }
                    });
                }
            }
        }
    }
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

#[tauri::command]
pub fn get_bookmarks(state: State<'_, AppState>) -> Result<Vec<crate::bookmarks::BookmarkItem>, String> {
    Ok(state.bookmark_manager.get_all())
}

#[tauri::command]
pub fn save_bookmark(
    state: State<'_, AppState>,
    title: String,
    url: String,
) -> Result<crate::bookmarks::BookmarkItem, String> {
    Ok(state.bookmark_manager.add(title, url))
}

#[tauri::command]
pub fn delete_bookmark(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    Ok(state.bookmark_manager.remove(&id))
}

#[tauri::command]
pub fn is_bookmarked(state: State<'_, AppState>, url: String) -> Result<bool, String> {
    let all = state.bookmark_manager.get_all();
    Ok(all.iter().any(|b| b.url == url))
}

fn parse_url_param(action: &str) -> Option<url::Url> {
    let full = format!("http://shadhin.action/{}", action);
    let parsed = url::Url::parse(&full).ok()?;
    let raw = parsed
        .query_pairs()
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.into_owned())?;
    if raw.trim().is_empty() { return None; }
    url::Url::parse(&raw).ok()
}

/// Central dispatcher for shortcuts and actions sent from injected content scripts
pub fn handle_injected_action(app: &AppHandle, action: &str) {
    let state = app.state::<AppState>();
    match action {
        "create_tab" => {
            let app_c = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = create_tab(app_c.clone(), None).await;
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
        "toggle_bookmarks" => {
            let _ = app.emit("trigger-shortcut", "toggle-bookmarks");
        }
        "open_settings" => {
            let _ = app.emit("trigger-shortcut", "open-settings");
        }
        "dismiss_popovers" => {
            if state.toolbar_expanded.load(Ordering::Relaxed) {
                if let Some(tb) = app.get_webview("toolbar") {
                    let _ = tb.eval("if (window.__closeAllPopovers) { window.__closeAllPopovers(); }");
                }
            }
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
        action if action.starts_with("on_fullscreen_element_changed") => {
            let is_fs = if let Some(q) = action.split('?').nth(1) {
                q.split('&').any(|p| p == "isFullscreen=true" || p == "is_fullscreen=true")
            } else {
                false
            };
            crate::fullscreen::handle_fullscreen_transition(app, is_fs);
        }
        action if action.starts_with("tracker_blocked_dom") => {
            if !state.ad_blocker.is_enabled() {
                return;
            }
            let count = if let Some(q) = action.split('?').nth(1) {
                q.split('&')
                    .find_map(|pair| {
                        let mut parts = pair.split('=');
                        if parts.next() == Some("count") {
                            parts.next().and_then(|v| v.parse::<usize>().ok())
                        } else {
                            None
                        }
                    })
                    .unwrap_or(1)
            } else {
                1
            };

            let total = state.ad_blocker.add_blocked(count);
            println!("[AdBlock] DOM Blocked via ping: +{} (total: {})", count, total);
            let _ = app.emit("tracker-blocked", serde_json::json!({
                "count": total
            }));
            if let Some(tb) = app.get_webview("toolbar") {
                let _ = tb.eval(&format!(
                    "if (window.__onTrackerBlocked) {{ window.__onTrackerBlocked({}); }}",
                    total
                ));
            }
        }
        action if action.starts_with("open_url_new_tab") => {
            if let Some(u) = parse_url_param(action) {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = create_tab(app_c, Some(u.to_string())).await;
                });
            }
        }
        action if action.starts_with("open_url_new_window") => {
            if let Some(u) = parse_url_param(action) {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = spawn_browser_window(app_c, u, false);
                });
            }
        }
        action if action.starts_with("open_url_private_window") => {
            if let Some(u) = parse_url_param(action) {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = spawn_browser_window(app_c, u, true);
                });
            }
        }
        _ => {}
    }
}

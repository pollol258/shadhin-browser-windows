pub mod blocker;
pub mod browser;
pub mod commands;
pub mod downloads;
pub mod fullscreen;
pub mod passwords;
pub mod settings;

use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowBuilder,
};

use crate::blocker::AdBlocker;
use crate::browser::tabs::TabManager;
use crate::browser::TOOLBAR_HEIGHT;
use crate::commands::AppState;
use crate::downloads::DownloadManager;
use crate::fullscreen::FullscreenManager;
use crate::passwords::PasswordManager;
use crate::settings::SettingsManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = dirs::data_dir()
        .map(|d| d.join("Shadhin-Browser"))
        .unwrap_or_else(|| std::path::PathBuf::from("./shadhin_data"));

    std::fs::create_dir_all(&app_data_dir).ok();

    let settings_manager = Arc::new(SettingsManager::new(app_data_dir.clone()));
    let settings = settings_manager.get();

    let password_manager = Arc::new(PasswordManager::new(app_data_dir.clone()));
    let download_manager = Arc::new(DownloadManager::new(app_data_dir.clone()));
    let ad_blocker = Arc::new(AdBlocker::new(settings.ad_block_enabled));
    let fullscreen_manager = Arc::new(FullscreenManager::new());
    let tab_manager = Arc::new(Mutex::new(TabManager::new()));

    let app_state = AppState {
        tab_manager,
        settings_manager,
        password_manager,
        download_manager,
        ad_blocker,
        fullscreen_manager,
        app_data_dir,
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::create_tab,
            commands::switch_tab,
            commands::close_tab,
            commands::close_active_tab,
            commands::navigate_to,
            commands::go_back,
            commands::go_forward,
            commands::reload,
            commands::stop,
            commands::get_tabs,
            commands::get_active_tab,
            commands::window_minimize,
            commands::window_maximize,
            commands::window_close,
            commands::window_start_dragging,
            commands::get_downloads,
            commands::open_download_file,
            commands::open_download_folder,
            commands::clear_downloads,
            commands::get_settings,
            commands::save_settings,
            commands::get_passwords,
            commands::save_password,
            commands::delete_password,
            commands::get_credentials_for_origin,
            commands::clear_browsing_data,
            commands::new_private_window,
            commands::toggle_adblock,
            commands::toggle_ad_blocker,
            commands::get_blocked_count,
            commands::on_login_submitted,
            commands::on_fullscreen_element_changed,
            commands::focus_address_bar,
            commands::toggle_devtools,
        ])
        .setup(|app| {
            let initial_width = 1200.0;
            let initial_height = 800.0;

            // Create frameless main window
            let window = WindowBuilder::new(app, "main")
                .title("Shadhin-Browser")
                .inner_size(initial_width, initial_height)
                .min_inner_size(600.0, 400.0)
                .decorations(false)
                .resizable(true)
                .shadow(true)
                .build()?;

            // Create top toolbar webview (Safari UI)
            let toolbar_builder = WebviewBuilder::new("toolbar", WebviewUrl::App("index.html".into()))
                .auto_resize();

            let _toolbar = window.add_child(
                toolbar_builder,
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(initial_width, TOOLBAR_HEIGHT),
            )?;

            // Window resize handler for adjusting child webviews
            let app_handle_for_resize: AppHandle = app.handle().clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::Resized(physical_size) = event {
                    let scale = match app_handle_for_resize.get_window("main") {
                        Some(w) => w.scale_factor().unwrap_or(1.0),
                        None => 1.0,
                    };
                    let width = physical_size.width as f64 / scale;
                    let height = physical_size.height as f64 / scale;
                    let content_height = (height - TOOLBAR_HEIGHT).max(10.0);

                    // Resize toolbar webview width
                    if let Some(tb) = app_handle_for_resize.get_webview("toolbar") {
                        let _ = tb.set_size(LogicalSize::new(width, TOOLBAR_HEIGHT));
                    }

                    // Resize active tab webview
                    let state = app_handle_for_resize.state::<AppState>();
                    if !state.fullscreen_manager.is_fullscreen() {
                        let active_label = {
                            let mgr = state.tab_manager.lock().unwrap();
                            mgr.get_active_tab().map(|t| t.label.clone())
                        };
                        if let Some(lbl) = active_label {
                            if let Some(wv) = app_handle_for_resize.get_webview(&lbl) {
                                let _ = wv.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
                                let _ = wv.set_size(LogicalSize::new(width, content_height));
                            }
                        }
                    }
                }
            });

            // Spawn background task to create first tab once UI initializes
            let app_clone = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                let state = app_clone.state::<AppState>();
                let _ = commands::create_tab(app_clone.clone(), state, None).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Shadhin-Browser");
}

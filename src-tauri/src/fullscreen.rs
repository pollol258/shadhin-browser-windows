use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Webview};

use crate::browser::TOOLBAR_HEIGHT;

pub struct FullscreenManager {
    is_fullscreen: Arc<AtomicBool>,
    was_maximized: Arc<AtomicBool>,
}

impl FullscreenManager {
    pub fn new() -> Self {
        Self {
            is_fullscreen: Arc::new(AtomicBool::new(false)),
            was_maximized: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen.load(Ordering::Relaxed)
    }

    pub fn set_fullscreen(&self, app: &AppHandle, active_label: Option<&str>, fullscreen: bool) -> anyhow::Result<()> {
        let _prev_state = self.is_fullscreen.swap(fullscreen, Ordering::Relaxed);

        let window = match app.get_window("main") {
            Some(w) => w,
            None => return Ok(()),
        };

        let toolbar = app.get_webview("toolbar");
        let active_webview = active_label
            .and_then(|lbl| app.get_webview(lbl))
            .or_else(|| {
                let state = app.state::<crate::commands::AppState>();
                let mgr = state.tab_manager.lock().unwrap();
                mgr.get_active_tab().and_then(|t| app.get_webview(&t.label))
            });

        if fullscreen {
            // Save maximized state before entering fullscreen
            let is_max = window.is_maximized().unwrap_or(false);
            self.was_maximized.store(is_max, Ordering::Relaxed);

            let (logical_width, logical_height) = if let Ok(Some(monitor)) = window.current_monitor() {
                let mon_size = monitor.size();
                let mon_scale = monitor.scale_factor();
                (mon_size.width as f64 / mon_scale, mon_size.height as f64 / mon_scale)
            } else if let Ok(size) = window.inner_size() {
                let scale_factor = window.scale_factor().unwrap_or(1.0);
                (size.width as f64 / scale_factor, size.height as f64 / scale_factor)
            } else {
                (1920.0, 1080.0)
            };

            // 1. Hide toolbar webview cleanly (NEVER collapse size to 0x0 as it breaks WebView2 renderer)
            if let Some(tb) = toolbar.as_ref() {
                let _ = tb.hide();
            }

            // 2. Transition main window to fullscreen
            let _ = window.set_fullscreen(true);

            // 3. Size active tab to full window (0, 0)
            if let Some(wv) = active_webview.as_ref() {
                let _ = wv.set_position(LogicalPosition::new(0.0, 0.0));
                let _ = wv.set_size(LogicalSize::new(logical_width, logical_height));
                let _ = wv.show();
            }
        } else {
            // 1. Exit fullscreen mode on main window
            let _ = window.set_fullscreen(false);

            // Restore maximized state if window was maximized before entering fullscreen
            let was_max = self.was_maximized.load(Ordering::Relaxed);
            if was_max {
                let _ = window.maximize();
            }

            // 2. Query current window size
            let (logical_width, logical_height) = if let Ok(size) = window.inner_size() {
                let scale_factor = window.scale_factor().unwrap_or(1.0);
                (size.width as f64 / scale_factor, size.height as f64 / scale_factor)
            } else {
                (1200.0, 800.0)
            };

            // 3. Show and position toolbar at top (0.0, 0.0)
            if let Some(tb) = toolbar.as_ref() {
                let _ = tb.set_position(LogicalPosition::new(0.0, 0.0));
                let _ = tb.set_size(LogicalSize::new(logical_width, TOOLBAR_HEIGHT));
                let _ = tb.show();
            }

            // 4. Restore active webview below toolbar (0.0, TOOLBAR_HEIGHT)
            let content_height = (logical_height - TOOLBAR_HEIGHT).max(10.0);
            if let Some(wv) = active_webview.as_ref() {
                let _ = wv.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
                let _ = wv.set_size(LogicalSize::new(logical_width, content_height));
                let _ = wv.show();
            }

            // 5. Multi-frame delayed re-sync to ensure layout is applied after OS window exit animation
            let app_c = app.clone();
            let active_lbl = active_label.map(|s| s.to_string());
            tauri::async_runtime::spawn(async move {
                for delay in [40, 100, 200, 350, 600] {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    let state = app_c.state::<crate::commands::AppState>();
                    if state.fullscreen_manager.is_fullscreen() {
                        return;
                    }
                    if let Some(win) = app_c.get_window("main") {
                        if was_max && !win.is_maximized().unwrap_or(false) {
                            let _ = win.maximize();
                        }
                        if let Ok(size) = win.inner_size() {
                            let scale = win.scale_factor().unwrap_or(1.0);
                            let w = size.width as f64 / scale;
                            let h = size.height as f64 / scale;
                            let ch = (h - TOOLBAR_HEIGHT).max(10.0);

                            if let Some(tb) = app_c.get_webview("toolbar") {
                                let _ = tb.set_position(LogicalPosition::new(0.0, 0.0));
                                let _ = tb.set_size(LogicalSize::new(w, TOOLBAR_HEIGHT));
                                let _ = tb.show();
                            }

                            let lbl = active_lbl.clone().or_else(|| {
                                let mgr = state.tab_manager.lock().unwrap();
                                mgr.get_active_tab().map(|t| t.label.clone())
                            });
                            if let Some(l) = lbl {
                                if let Some(wv) = app_c.get_webview(&l) {
                                    let _ = wv.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
                                    let _ = wv.set_size(LogicalSize::new(w, ch));
                                    let _ = wv.show();
                                }
                            }
                        }
                    }
                }
            });
        }

        Ok(())
    }
}

pub fn handle_fullscreen_transition(app: &AppHandle, is_fullscreen: bool) {
    println!("[Fullscreen] state changed -> {}", is_fullscreen);
    let state = app.state::<crate::commands::AppState>();
    let active_label = {
        let mgr = state.tab_manager.lock().unwrap();
        mgr.get_active_tab().map(|t| t.label.clone())
    };
    let _ = state
        .fullscreen_manager
        .set_fullscreen(app, active_label.as_deref(), is_fullscreen);
}

#[cfg(windows)]
pub fn hook_tab_fullscreen(
    app: AppHandle,
    webview: &Webview,
) -> anyhow::Result<()> {
    let tab_label = webview.label().to_string();
    webview.with_webview(move |platform_webview| {
        unsafe {
            let controller = platform_webview.controller();
            let Ok(core) = controller.CoreWebView2() else { return; };
            let mut token = Default::default();
            let app_h = app.clone();
            let label = tab_label.clone();

            let handler = webview2_com::ContainsFullScreenElementChangedEventHandler::create(Box::new(
                move |sender, _args| {
                    let Some(core) = sender else { return Ok(()); };
                    let mut contains = windows_core::BOOL(0);
                    core.ContainsFullScreenElement(&mut contains)?;
                    let is_fullscreen = contains.as_bool();
                    println!("[Fullscreen] Native ContainsFullScreenElementChanged -> {}", is_fullscreen);

                    let app_c = app_h.clone();
                    let lbl_c = label.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_c.state::<crate::commands::AppState>();
                        let _ = state.fullscreen_manager.set_fullscreen(&app_c, Some(&lbl_c), is_fullscreen);
                    });

                    Ok(())
                },
            ));

            let _ = core.add_ContainsFullScreenElementChanged(&handler, &mut token);
        }
    })?;
    Ok(())
}

#[cfg(not(windows))]
pub fn hook_tab_fullscreen(
    _app: AppHandle,
    _webview: &Webview,
) -> anyhow::Result<()> {
    Ok(())
}

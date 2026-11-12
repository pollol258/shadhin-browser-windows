use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager};

use crate::browser::TOOLBAR_HEIGHT;

pub struct FullscreenManager {
    is_fullscreen: Arc<AtomicBool>,
}

impl FullscreenManager {
    pub fn new() -> Self {
        Self {
            is_fullscreen: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen.load(Ordering::Relaxed)
    }

    pub fn set_fullscreen(&self, app: &AppHandle, active_label: Option<&str>, fullscreen: bool) -> anyhow::Result<()> {
        self.is_fullscreen.store(fullscreen, Ordering::Relaxed);

        let window = match app.get_window("main") {
            Some(w) => w,
            None => return Ok(()),
        };

        window.set_fullscreen(fullscreen)?;

        let toolbar = app.get_webview("toolbar");
        let active_webview = active_label.and_then(|lbl| app.get_webview(lbl));

        let size = window.inner_size()?;
        let scale_factor = window.scale_factor().unwrap_or(1.0);
        let logical_width = size.width as f64 / scale_factor;
        let logical_height = size.height as f64 / scale_factor;

        if fullscreen {
            // Hide toolbar
            if let Some(tb) = toolbar {
                let _ = tb.hide();
            }
            // Expand active webview to full window
            if let Some(wv) = active_webview {
                let _ = wv.set_position(LogicalPosition::new(0.0, 0.0));
                let _ = wv.set_size(LogicalSize::new(logical_width, logical_height));
            }
        } else {
            // Show toolbar
            if let Some(tb) = toolbar {
                let _ = tb.show();
                let _ = tb.set_position(LogicalPosition::new(0.0, 0.0));
                let _ = tb.set_size(LogicalSize::new(logical_width, TOOLBAR_HEIGHT));
            }
            // Restore active webview below toolbar
            if let Some(wv) = active_webview {
                let _ = wv.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
                let content_height = (logical_height - TOOLBAR_HEIGHT).max(10.0);
                let _ = wv.set_size(LogicalSize::new(logical_width, content_height));
            }
        }

        Ok(())
    }
}

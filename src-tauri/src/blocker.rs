use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Webview};

#[cfg(windows)]
use webview2_com::Microsoft::Web::WebView2::Win32::*;
#[cfg(windows)]
use windows_core::Interface;

pub struct AdBlocker {
    enabled: Arc<AtomicBool>,
    blocked_count: Arc<AtomicUsize>,
    blocked_domains: HashSet<&'static str>,
}

impl AdBlocker {
    pub fn new(enabled: bool) -> Self {
        let mut domains = HashSet::new();

        // High-profile advertising, telemetry, tracker networks (EasyList-lite & hosts)
        let default_list = &[
            // Google Ads / DoubleClick / Analytics / Tag Manager
            "doubleclick.net",
            "adservice.google.com",
            "googleads.g.doubleclick.net",
            "googlesyndication.com",
            "pagead2.googlesyndication.com",
            "google-analytics.com",
            "analytics.google.com",
            "googletagmanager.com",
            "googletagservices.com",
            // Facebook / Meta trackers
            "pixel.facebook.com",
            "an.facebook.com",
            "connect.facebook.net",
            // Microsoft / Yahoo / Bing
            "bat.bing.com",
            "adnxs.com",
            "ib.adnxs.com",
            "ads.yahoo.com",
            "analytics.yahoo.com",
            // Amazon
            "amazon-adsystem.com",
            "aax.amazon-adsystem.com",
            // Major ad exchanges & SSP/DSP
            "criteo.com",
            "criteo.net",
            "static.criteo.net",
            "rubiconproject.com",
            "pubmatic.com",
            "openx.net",
            "casalemedia.com",
            "taboola.com",
            "outbrain.com",
            "smartadserver.com",
            "yieldmo.com",
            "bidswitch.net",
            "scorecardresearch.com",
            "quantserve.com",
            "moatads.com",
            "serving-sys.com",
            "adroll.com",
            "advertising.com",
            "advertising.apple.com",
            "media.net",
            "sovrn.com",
            "sharethrough.com",
            "triplelift.com",
            "revcontent.com",
            "mgid.com",
            "infolinks.com",
            "popcash.net",
            "popads.net",
            "propellerads.com",
            "adsterra.com",
            // Social analytics / Trackers
            "analytics.tiktok.com",
            "ads-twitter.com",
            "analytics.twitter.com",
            "ads.linkedin.com",
            "snap.licdn.com",
            // User tracking / heatmaps / Telemetry
            "hotjar.com",
            "static.hotjar.com",
            "clarity.ms",
            "mixpanel.com",
            "api.mixpanel.com",
            "segment.io",
            "cdn.segment.com",
            "newrelic.com",
            // Cross-site fingerprinting / Cookie sync
            "cdn.krxd.net",
            "demdex.net",
            "bluekai.com",
            "exelator.com",
            "agkn.com",
            "rlcdn.com",
            "tapad.com",
            "id5-sync.com",
            // Crypto miners / malicious scripts
            "coinhive.com",
            "coin-hive.com",
        ];

        for d in default_list {
            domains.insert(*d);
        }

        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
            blocked_count: Arc::new(AtomicUsize::new(0)),
            blocked_domains: domains,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::Relaxed);
    }

    pub fn get_blocked_count(&self) -> usize {
        self.blocked_count.load(Ordering::Relaxed)
    }

    pub fn add_blocked(&self, count: usize) -> usize {
        self.blocked_count.fetch_add(count, Ordering::Relaxed) + count
    }

    pub fn record_blocked(&self) -> usize {
        self.blocked_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn should_block(&self, url_str: &str) -> bool {
        if !self.is_enabled() {
            return false;
        }

        if url_str.starts_with("tauri://")
            || url_str.starts_with("http://tauri.localhost")
            || url_str.starts_with("https://tauri.localhost")
        {
            return false;
        }

        // Parse host from URL
        let host = if let Ok(parsed) = url::Url::parse(url_str) {
            match parsed.host_str() {
                Some(h) => h.to_lowercase(),
                None => return false,
            }
        } else {
            return false;
        };

        // Exact match or subdomain match without extra allocations
        if self.blocked_domains.contains(host.as_str()) {
            return true;
        }

        for domain in &self.blocked_domains {
            if host.ends_with(domain) {
                if host.len() == domain.len()
                    || host.as_bytes().get(host.len() - domain.len() - 1) == Some(&b'.')
                {
                    return true;
                }
            }
        }

        // Common tracker/telemetry path & query patterns
        let lower = url_str.to_lowercase();
        let tracker_patterns = &[
            "/pagead/",
            "/ads.js",
            "/google-analytics.com/",
            "/analytics.js",
            "/gtm.js",
            "facebook.com/tr",
            "/fbevents.js",
            "clarity.ms/tag",
            "stats.g.doubleclick.net",
            "/pixel.gif",
            "/beacon.js",
            "/telemetry",
            "scorecardresearch.com",
            "taboola.com",
            "outbrain.com",
            "criteo.com",
            "adnxs.com",
            "googlesyndication.com",
        ];

        for pattern in tracker_patterns {
            if lower.contains(pattern) {
                return true;
            }
        }

        false
    }

    pub fn is_blocked(&self, url_str: &str) -> bool {
        self.should_block(url_str)
    }
}

// ==========================================
// Native Tab Blocker Hook (WebView2 COM)
// ==========================================

#[cfg(windows)]
pub fn hook_tab_blocker(
    app: AppHandle,
    webview: &Webview,
    blocker: Arc<AdBlocker>,
) -> anyhow::Result<()> {
    webview.with_webview(move |platform_webview| {
        unsafe {
            let controller = platform_webview.controller();
            let Ok(core) = controller.CoreWebView2() else { return; };
            let Ok(core2) = core.cast::<ICoreWebView2_2>() else { return; };
            let Ok(env) = core2.Environment() else { return; };

            const STAR_U16: &[u16] = &[b'*' as u16, 0];
            let _ = core.AddWebResourceRequestedFilter(
                windows_core::PCWSTR(STAR_U16.as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            );

            // WebResourceRequested event handler
            let mut req_token = Default::default();
            let blocker_req = blocker.clone();
            let app_req = app.clone();
            let env_req = env.clone();

            const NO_CONTENT_U16: &[u16] = &[b'N' as u16, b'o' as u16, b' ' as u16, b'C' as u16, b'o' as u16, b'n' as u16, b't' as u16, b'e' as u16, b'n' as u16, b't' as u16, 0];
            const BLOCKED_U16: &[u16] = &[b'B' as u16, b'l' as u16, b'o' as u16, b'c' as u16, b'k' as u16, b'e' as u16, b'd' as u16, 0];
            const EMPTY_U16: &[u16] = &[0];

            let req_handler = webview2_com::WebResourceRequestedEventHandler::create(Box::new(
                move |_sender, args| {
                    let Some(args) = args else { return Ok(()); };
                    let req = args.Request()?;
                    let uri_str = {
                        let mut uri = windows_core::PWSTR(std::ptr::null_mut());
                        if req.Uri(&mut uri).is_ok() && !uri.0.is_null() {
                            webview2_com::take_pwstr(uri)
                        } else {
                            String::new()
                        }
                    };

                    if uri_str.is_empty() {
                        return Ok(());
                    }

                    // Skip interception on internal/local Tauri app assets
                    if uri_str.starts_with("tauri://")
                        || uri_str.starts_with("http://tauri.localhost")
                        || uri_str.starts_with("https://tauri.localhost")
                    {
                        return Ok(());
                    }

                    // Intercept internal action pings (http://shadhin.action/...)
                    if uri_str.starts_with("http://shadhin.action/") {
                        if let Ok(resp) = env_req.CreateWebResourceResponse(
                            None,
                            204,
                            windows_core::PCWSTR(NO_CONTENT_U16.as_ptr()),
                            windows_core::PCWSTR(EMPTY_U16.as_ptr()),
                        ) {
                            let _ = args.SetResponse(&resp);
                        }
                        let path = uri_str.strip_prefix("http://shadhin.action/").unwrap_or_default().to_string();
                        crate::commands::handle_injected_action(&app_req, &path);
                        return Ok(());
                    }

                    // Check adblock
                    if blocker_req.is_enabled() && blocker_req.should_block(&uri_str) {
                        if let Ok(resp) = env_req.CreateWebResourceResponse(
                            None,
                            403,
                            windows_core::PCWSTR(BLOCKED_U16.as_ptr()),
                            windows_core::PCWSTR(EMPTY_U16.as_ptr()),
                        ) {
                            let _ = args.SetResponse(&resp);
                        }

                        let count = blocker_req.record_blocked();
                        println!("[AdBlock] Blocked: {} (total: {})", uri_str, count);

                        let _ = app_req.emit("tracker-blocked", serde_json::json!({
                            "count": count,
                            "url": uri_str,
                        }));

                        if let Some(tb) = app_req.get_webview("toolbar") {
                            let _ = tb.eval(&format!(
                                "if (window.__onTrackerBlocked) {{ window.__onTrackerBlocked({}); }}",
                                count
                            ));
                        }
                    }

                    Ok(())
                },
            ));

            let _ = core.add_WebResourceRequested(&req_handler, &mut req_token);

            // WebMessageReceived event handler for DOM blocker & injected messages
            let mut msg_token = Default::default();
            let blocker_msg = blocker.clone();
            let app_msg = app.clone();

            let msg_handler = webview2_com::WebMessageReceivedEventHandler::create(Box::new(
                move |_sender, args| {
                    let Some(args) = args else { return Ok(()); };
                    let msg_str = {
                        let mut msg_pwstr = windows_core::PWSTR(std::ptr::null_mut());
                        if args.TryGetWebMessageAsString(&mut msg_pwstr).is_ok() && !msg_pwstr.0.is_null() {
                            webview2_com::take_pwstr(msg_pwstr)
                        } else {
                            let mut json_pwstr = windows_core::PWSTR(std::ptr::null_mut());
                            if args.WebMessageAsJson(&mut json_pwstr).is_ok() && !json_pwstr.0.is_null() {
                                webview2_com::take_pwstr(json_pwstr)
                            } else {
                                String::new()
                            }
                        }
                    };

                    if !msg_str.is_empty() {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg_str) {
                            let action = val.get("action").or_else(|| val.get("type")).and_then(|v| v.as_str());
                            if let Some(act) = action {
                                if act == "tracker_blocked_dom" {
                                    if blocker_msg.is_enabled() {
                                        let c = val.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                                        let total = blocker_msg.add_blocked(c);
                                        println!("[AdBlock] DOM Blocked (message): +{} (total: {})", c, total);
                                        let _ = app_msg.emit("tracker-blocked", serde_json::json!({
                                            "count": total
                                        }));
                                        if let Some(tb) = app_msg.get_webview("toolbar") {
                                            let _ = tb.eval(&format!(
                                                "if (window.__onTrackerBlocked) {{ window.__onTrackerBlocked({}); }}",
                                                total
                                            ));
                                        }
                                    }
                                } else if act.starts_with("on_fullscreen_element_changed") {
                                    let parse_bool_val = |v: &serde_json::Value| -> Option<bool> {
                                        if let Some(b) = v.as_bool() {
                                            Some(b)
                                        } else if let Some(s) = v.as_str() {
                                            Some(s == "true" || s == "1")
                                        } else {
                                            None
                                        }
                                    };
                                    let is_fs = val.get("isFullscreen")
                                        .and_then(parse_bool_val)
                                        .or_else(|| val.get("data").and_then(|d| d.get("isFullscreen")).and_then(parse_bool_val))
                                        .unwrap_or_else(|| {
                                            act.split('?').nth(1).map(|q| {
                                                q.split('&').any(|p| p == "isFullscreen=true" || p == "is_fullscreen=true" || p == "isFullscreen=1")
                                            }).unwrap_or(false)
                                        });
                                    let app_fs = app_msg.clone();
                                    tauri::async_runtime::spawn(async move {
                                        crate::fullscreen::handle_fullscreen_transition(&app_fs, is_fs);
                                    });
                                } else {
                                    crate::commands::handle_injected_action(&app_msg, act);
                                }
                            }
                        }
                    }

                    Ok(())
                },
            ));

            let _ = core.add_WebMessageReceived(&msg_handler, &mut msg_token);
        }
    })?;
    Ok(())
}

#[cfg(not(windows))]
pub fn hook_tab_blocker(
    _app: AppHandle,
    _webview: &Webview,
    _blocker: Arc<AdBlocker>,
) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ad_tracker_blocking() {
        let blocker = AdBlocker::new(true);

        // Known ad/tracker domains and subdomains
        assert!(blocker.is_blocked("https://adservice.google.com/ads/test"));
        assert!(blocker.is_blocked("https://googleads.g.doubleclick.net/pagead/id"));
        assert!(blocker.is_blocked("https://stats.g.doubleclick.net/dc.js"));
        assert!(blocker.is_blocked("https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js"));
        assert!(blocker.is_blocked("https://www.google-analytics.com/analytics.js"));
        assert!(blocker.is_blocked("https://pixel.facebook.com/tr/"));
        assert!(blocker.is_blocked("https://criteo.com/delivery/"));
        assert!(blocker.is_blocked("https://static.criteo.net/js/ld.js"));
        assert!(blocker.is_blocked("https://secure.adnxs.com/seg?add=1"));

        // Path patterns
        assert!(blocker.is_blocked("https://example.com/pagead/ad_frame.html"));
        assert!(blocker.is_blocked("https://cdn.site.org/assets/ads.js"));
        assert!(blocker.is_blocked("https://cdn.site.org/lib/gtm.js"));
    }

    #[test]
    fn test_legitimate_urls_not_blocked() {
        let blocker = AdBlocker::new(true);

        assert!(!blocker.is_blocked("https://www.google.com/search?q=rust"));
        assert!(!blocker.is_blocked("https://github.com/pollol258/Shadhin-Browser"));
        assert!(!blocker.is_blocked("https://www.wikipedia.org/wiki/Rust"));
        assert!(!blocker.is_blocked("https://stackoverflow.com/questions/12345"));
        assert!(!blocker.is_blocked("https://tauri.app/"));
        assert!(!blocker.is_blocked("tauri://localhost/index.html"));
        assert!(!blocker.is_blocked("http://tauri.localhost/newtab.html"));
    }

    #[test]
    fn test_counter_and_toggle() {
        let blocker = AdBlocker::new(true);
        assert_eq!(blocker.get_blocked_count(), 0);

        assert_eq!(blocker.record_blocked(), 1);
        assert_eq!(blocker.get_blocked_count(), 1);

        assert_eq!(blocker.add_blocked(4), 5);
        assert_eq!(blocker.get_blocked_count(), 5);

        // Disabling blocker
        blocker.set_enabled(false);
        assert!(!blocker.is_enabled());
        assert!(!blocker.is_blocked("https://googleads.g.doubleclick.net"));
        assert!(!blocker.is_blocked("https://www.google-analytics.com/analytics.js"));

        blocker.set_enabled(true);
        assert!(blocker.is_enabled());
        assert!(blocker.is_blocked("https://googleads.g.doubleclick.net"));
    }
}

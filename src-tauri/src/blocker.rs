use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

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

    pub fn is_blocked(&self, url_str: &str) -> bool {
        if !self.is_enabled() {
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

        // Exact match or subdomain match
        if self.blocked_domains.contains(host.as_str()) {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            return true;
        }

        for domain in &self.blocked_domains {
            if host.ends_with(&format!(".{}", domain)) {
                self.blocked_count.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }

        false
    }
}

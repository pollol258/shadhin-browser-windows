pub mod navigation;
pub mod tabs;

pub const TOOLBAR_HEIGHT: f64 = 82.0;

pub const PAGE_INJECTION_SCRIPT: &str = r#"
(function() {
    // Skip injection on internal / local pages
    try {
        if (window.location.protocol === 'tauri:' ||
            window.location.hostname === 'tauri.localhost' ||
            window.location.pathname.indexOf('newtab.html') >= 0) {
            return;
        }
    } catch (e) {}

    // Helper to send actions back to Rust host reliably across all origins/CSPs
    function sendAction(action, data) {
        var baseAction = action;
        var payload = data || {};
        if (typeof action === 'string' && action.indexOf('?') >= 0) {
            var parts = action.split('?');
            baseAction = parts[0];
            var q = parts[1];
            var pairs = q.split('&');
            for (var i = 0; i < pairs.length; i++) {
                var kv = pairs[i].split('=');
                if (kv[0] && kv[1] !== undefined) {
                    var val = decodeURIComponent(kv[1]);
                    if (val === 'true') {
                        payload[kv[0]] = true;
                    } else if (val === 'false') {
                        payload[kv[0]] = false;
                    } else {
                        payload[kv[0]] = isNaN(Number(val)) ? val : Number(val);
                    }
                }
            }
        }

        var sent = false;
        try {
            if (window.chrome && window.chrome.webview && window.chrome.webview.postMessage) {
                var isFsVal = payload.isFullscreen;
                if (typeof isFsVal === 'string') {
                    isFsVal = (isFsVal === 'true' || isFsVal === '1');
                } else if (typeof isFsVal !== 'boolean') {
                    isFsVal = Boolean(isFsVal);
                }
                window.chrome.webview.postMessage(JSON.stringify({
                    type: baseAction,
                    action: action,
                    isFullscreen: isFsVal,
                    count: payload.count || 1,
                    data: payload
                }));
                sent = true;
            }
        } catch (e) {}

        if (!sent) {
            try {
                if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                    window.__TAURI_INTERNALS__.invoke(baseAction, payload);
                    sent = true;
                }
            } catch (e) {}
        }

        if (!sent) {
            try {
                var url = 'http://shadhin.action/' + action;
                if (data && action.indexOf('?') < 0) {
                    url += (url.indexOf('?') >= 0 ? '&' : '?') + 'd=' + encodeURIComponent(JSON.stringify(data));
                }
                var ping = new Image();
                ping.src = url;
                if (window.fetch) {
                    fetch(url, { mode: 'no-cors' }).catch(function() {});
                }
            } catch (e) {}
        }
    }

    // 0. Cosmetic Ad & Tracker blocker element hiding & counting
    function scanAndBlockAds() {
        var adSelectors = [
            '.adsbygoogle',
            '[id^="google_ads_iframe"]',
            '[id^="div-gpt-ad"]',
            '.ad-container',
            '.ad-banner',
            '.advertisement',
            '.ad-slot',
            'iframe[src*="doubleclick"]',
            'iframe[src*="googlesyndication"]',
            'iframe[src*="criteo"]',
            'iframe[src*="adnxs"]',
            '.taboola-feed',
            '#taboola-below-article-thumbnails',
            '.outbrain_widget',
            '.OUTBRAIN',
            'div[data-ad]',
            'div[data-ad-client]',
            'div[data-ad-slot]'
        ];
        try {
            var elements = document.querySelectorAll(adSelectors.join(','));
            var count = 0;
            for (var i = 0; i < elements.length; i++) {
                var el = elements[i];
                if (!el.dataset.shadhinBlocked) {
                    el.dataset.shadhinBlocked = 'true';
                    el.style.setProperty('display', 'none', 'important');
                    el.style.setProperty('visibility', 'hidden', 'important');
                    el.style.setProperty('height', '0', 'important');
                    el.style.setProperty('max-height', '0', 'important');
                    el.style.setProperty('opacity', '0', 'important');
                    el.style.setProperty('pointer-events', 'none', 'important');
                    count++;
                }
            }
            if (count > 0) {
                sendAction('tracker_blocked_dom?count=' + count, { count: count });
            }
        } catch(e) {}
    }

    try {
        var adStyle = document.createElement('style');
        adStyle.id = 'shadhin-adblock-styles';
        adStyle.textContent = '.adsbygoogle,[id^="google_ads_iframe"],[id^="div-gpt-ad"],.ad-container,.ad-banner,.advertisement,.ad-slot,iframe[src*="doubleclick"],iframe[src*="googlesyndication"],iframe[src*="criteo"],iframe[src*="adnxs"],.taboola-feed,#taboola-below-article-thumbnails,.outbrain_widget,.OUTBRAIN,div[data-ad],div[data-ad-client],div[data-ad-slot]{display:none!important;visibility:hidden!important;height:0!important;max-height:0!important;opacity:0!important;pointer-events:none!important;}';
        (document.head || document.documentElement).appendChild(adStyle);
    } catch(e) {}

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', scanAndBlockAds);
    } else {
        scanAndBlockAds();
    }
    window.addEventListener('load', scanAndBlockAds);

    try {
        var obs = new MutationObserver(function() {
            scanAndBlockAds();
        });
        obs.observe(document.documentElement || document.body, { childList: true, subtree: true });
    } catch(e) {}

    // 1. Password Manager: Intercept form submits with passwords
    document.addEventListener('submit', function(e) {
        try {
            var form = e.target;
            if (!form || !form.elements) return;
            var passwordField = null;
            var usernameField = null;

            for (var i = 0; i < form.elements.length; i++) {
                var el = form.elements[i];
                if (!el || !el.type) continue;
                if (el.type.toLowerCase() === 'password' && !passwordField) {
                    passwordField = el;
                }
            }

            if (passwordField && passwordField.value) {
                // Find nearest text/email input for username
                for (var j = 0; j < form.elements.length; j++) {
                    var input = form.elements[j];
                    if (!input || !input.type) continue;
                    var type = input.type.toLowerCase();
                    var name = (input.name || '').toLowerCase();
                    if ((type === 'text' || type === 'email' || name.includes('user') || name.includes('login') || name.includes('email')) && input.value) {
                        usernameField = input;
                        break;
                    }
                }

                var origin = window.location.origin;
                var username = usernameField ? usernameField.value : '';
                var password = passwordField.value;

                if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                    window.__TAURI_INTERNALS__.invoke('on_login_submitted', {
                        origin: origin,
                        username: username,
                        password: password
                    });
                }
            }
        } catch (err) {
            console.error('Password detection error:', err);
        }
    }, true);

    // 2. Autofill credentials if available for this origin
    function tryAutofill() {
        if (!window.__TAURI_INTERNALS__ || !window.__TAURI_INTERNALS__.invoke) return;
        window.__TAURI_INTERNALS__.invoke('get_credentials_for_origin', {
            origin: window.location.origin
        }).then(function(creds) {
            if (creds && creds.length === 2) {
                var username = creds[0];
                var password = creds[1];

                var passInputs = document.querySelectorAll('input[type="password"]');
                if (passInputs.length > 0) {
                    var passInput = passInputs[0];
                    if (!passInput.value) {
                        passInput.value = password;
                        passInput.dispatchEvent(new Event('input', { bubbles: true }));
                        passInput.dispatchEvent(new Event('change', { bubbles: true }));
                    }

                    // Look for preceding text/email input
                    var form = passInput.form;
                    if (form) {
                        var textInputs = form.querySelectorAll('input[type="text"], input[type="email"]');
                        if (textInputs.length > 0 && !textInputs[0].value) {
                            textInputs[0].value = username;
                            textInputs[0].dispatchEvent(new Event('input', { bubbles: true }));
                            textInputs[0].dispatchEvent(new Event('change', { bubbles: true }));
                        }
                    }
                }
            }
        }).catch(function(err) {});
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', tryAutofill);
    } else {
        tryAutofill();
    }

    // 3. HTML5 Fullscreen Listener (YouTube, Vimeo, Facebook video)
    var lastFsState = null;
    function onFsChange() {
        var isFs = !!(document.fullscreenElement || document.webkitFullscreenElement || document.mozFullScreenElement || document.msFullscreenElement);
        if (lastFsState === isFs) return;
        lastFsState = isFs;
        sendAction('on_fullscreen_element_changed?isFullscreen=' + isFs, { isFullscreen: isFs });
    }
    document.addEventListener('fullscreenchange', onFsChange);
    document.addEventListener('webkitfullscreenchange', onFsChange);
    document.addEventListener('mozfullscreenchange', onFsChange);
    document.addEventListener('MSFullscreenChange', onFsChange);

    // 4. Keyboard Shortcuts forwarder (Capture phase)
    window.addEventListener('keydown', function(e) {
        var isCtrl = e.ctrlKey || e.metaKey;
        var key = e.key;
        var code = e.code || '';

        // Ctrl+T: New Tab
        if (isCtrl && !e.shiftKey && (key === 't' || key === 'T' || code === 'KeyT')) {
            e.preventDefault();
            sendAction('create_tab');
        }
        // Ctrl+W: Close Active Tab
        else if (isCtrl && !e.shiftKey && (key === 'w' || key === 'W' || code === 'KeyW')) {
            e.preventDefault();
            sendAction('close_active_tab');
        }
        // Ctrl+R / F5: Reload
        else if ((isCtrl && !e.shiftKey && (key === 'r' || key === 'R' || code === 'KeyR')) || (key === 'F5' && !e.ctrlKey)) {
            e.preventDefault();
            sendAction('reload');
        }
        // Ctrl+Shift+R / Ctrl+F5: Hard Reload
        else if ((isCtrl && e.shiftKey && (key === 'r' || key === 'R' || code === 'KeyR')) || (isCtrl && key === 'F5')) {
            e.preventDefault();
            sendAction('reload');
        }
        // Ctrl+L / Alt+D / F6: Focus Address Bar
        else if ((isCtrl && (key === 'l' || key === 'L' || code === 'KeyL')) || (e.altKey && (key === 'd' || key === 'D' || code === 'KeyD')) || key === 'F6') {
            e.preventDefault();
            sendAction('focus_address_bar');
        }
        // Alt + Left: Back
        else if (e.altKey && (key === 'ArrowLeft' || code === 'ArrowLeft')) {
            e.preventDefault();
            sendAction('go_back');
        }
        // Alt + Right: Forward
        else if (e.altKey && (key === 'ArrowRight' || code === 'ArrowRight')) {
            e.preventDefault();
            sendAction('go_forward');
        }
        // Ctrl+Shift+N: New Private Window
        else if (isCtrl && e.shiftKey && (key === 'n' || key === 'N' || code === 'KeyN')) {
            e.preventDefault();
            sendAction('new_private_window');
        }
        // Ctrl+Tab / Ctrl+PageDown: Next Tab
        else if ((isCtrl && !e.shiftKey && (key === 'Tab' || code === 'Tab')) || (isCtrl && (key === 'PageDown' || code === 'PageDown'))) {
            e.preventDefault();
            sendAction('switch_next_tab');
        }
        // Ctrl+Shift+Tab / Ctrl+PageUp: Previous Tab
        else if ((isCtrl && e.shiftKey && (key === 'Tab' || code === 'Tab')) || (isCtrl && (key === 'PageUp' || code === 'PageUp'))) {
            e.preventDefault();
            sendAction('switch_prev_tab');
        }
        // Ctrl+1 .. Ctrl+8: Tab N
        else if (isCtrl && key >= '1' && key <= '8') {
            e.preventDefault();
            sendAction('switch_tab_' + key);
        }
        // Ctrl+9: Last Tab
        else if (isCtrl && key === '9') {
            e.preventDefault();
            sendAction('switch_last_tab');
        }
        // Ctrl+Home: First Tab
        else if (isCtrl && (key === 'Home' || code === 'Home')) {
            e.preventDefault();
            sendAction('switch_first_tab');
        }
        // Ctrl+End: Last Tab
        else if (isCtrl && (key === 'End' || code === 'End')) {
            e.preventDefault();
            sendAction('switch_last_tab');
        }
        // Ctrl+J: Downloads
        else if (isCtrl && (key === 'j' || key === 'J' || code === 'KeyJ')) {
            e.preventDefault();
            sendAction('toggle_downloads');
        }
        // Ctrl+B: Bookmarks
        else if (isCtrl && (key === 'b' || key === 'B' || code === 'KeyB')) {
            e.preventDefault();
            sendAction('toggle_bookmarks');
        }
        // Ctrl+, / Ctrl+H: Settings
        else if (isCtrl && (key === ',' || key === 'h' || key === 'H' || code === 'KeyH')) {
            e.preventDefault();
            sendAction('open_settings');
        }
        // Ctrl + '+' / '=': Zoom In
        else if (isCtrl && (key === '=' || key === '+' || code === 'Equal' || code === 'NumpadAdd')) {
            e.preventDefault();
            pageZoomLevel = Math.min(3.0, Math.round((pageZoomLevel + 0.1) * 10) / 10);
            document.documentElement.style.zoom = pageZoomLevel;
        }
        // Ctrl + '-': Zoom Out
        else if (isCtrl && (key === '-' || code === 'Minus' || code === 'NumpadSubtract')) {
            e.preventDefault();
            pageZoomLevel = Math.max(0.3, Math.round((pageZoomLevel - 0.1) * 10) / 10);
            document.documentElement.style.zoom = pageZoomLevel;
        }
        // Ctrl + '0': Reset Zoom
        else if (isCtrl && (key === '0' || code === 'Digit0' || code === 'Numpad0')) {
            e.preventDefault();
            pageZoomLevel = 1.0;
            document.documentElement.style.zoom = '1';
        }
        // Esc: Exit Fullscreen / Stop Loading
        else if (key === 'Escape' || code === 'Escape') {
            var hasFs = !!(document.fullscreenElement || document.webkitFullscreenElement || document.mozFullScreenElement || document.msFullscreenElement);
            if (hasFs) {
                try {
                    (document.exitFullscreen || document.webkitExitFullscreen || document.mozCancelFullScreen || document.msExitFullscreen).call(document);
                } catch (err) {}
            }
            sendAction('on_fullscreen_element_changed?isFullscreen=false', { isFullscreen: false });
            if (!hasFs) {
                sendAction('stop');
            }
        }
    }, true);

    // 5. Ctrl + Mouse Wheel Zoom
    var pageZoomLevel = 1.0;
    window.addEventListener('wheel', function(e) {
        if (e.ctrlKey) {
            e.preventDefault();
            if (e.deltaY < 0) {
                // Scroll up -> Zoom In
                pageZoomLevel = Math.min(3.0, Math.round((pageZoomLevel + 0.1) * 10) / 10);
            } else {
                // Scroll down -> Zoom Out
                pageZoomLevel = Math.max(0.3, Math.round((pageZoomLevel - 0.1) * 10) / 10);
            }
            document.documentElement.style.zoom = pageZoomLevel;
        }
    }, { passive: false });

    // 6. Download links & target="_blank" safety handler
    document.addEventListener('click', function(e) {
        var a = e.target.closest('a');
        if (!a || !a.href) return;
        var href = a.href;
        var hasDownload = a.hasAttribute('download');
        var isDownloadFile = /\.(zip|exe|tar|gz|pdf|mp3|mp4|apk|iso|dmg|msi|rar|7z|csv|xlsx|docx|bin|dmg|pkg)(\?.*)?$/i.test(href);
        if (a.target === '_blank' && (hasDownload || isDownloadFile)) {
            a.target = '_self';
        }
    }, true);

    // 7. Tab-side outside click dismissal for toolbar popovers (Capture phase)
    window.addEventListener('mousedown', function(e) {
        sendAction('dismiss_popovers');
    }, true);

    // ==================================================================
    // 8. Custom context menu on links (Open in Tab, Window, Private)
    // ==================================================================
    (function () {
        if (window.__shadhinCtxInstalled) return;
        window.__shadhinCtxInstalled = true;

        var menuHost = null;

        function closeMenu() {
            if (menuHost) { menuHost.remove(); menuHost = null; }
        }

        function getLink(el) {
            while (el && el !== document) {
                if (el.tagName === 'A') return el;
                el = el.parentNode;
            }
            return null;
        }

        function isWebLink(a) {
            return !!(a && a.href && /^https?:/i.test(a.href));
        }

        function isDark() {
            try {
                return window.matchMedia('(prefers-color-scheme: dark)').matches;
            } catch (_) { return false; }
        }

        function sendOpen(kind, href) {
            var url = 'http://shadhin.action/' + kind + '?url=' + encodeURIComponent(href);
            try {
                // Prefer the same transport the rest of the script uses.
                if (typeof sendAction === 'function') {
                    sendAction(kind + '?url=' + encodeURIComponent(href));
                } else {
                    fetch(url, { mode: 'no-cors' });
                }
            } catch (_) {}
        }

        function showMenu(x, y, href) {
            closeMenu();
            var dark = isDark();
            var bg    = dark ? '#2b2b2d' : '#ffffff';
            var fg    = dark ? '#e8e8ea' : '#111111';
            var bd    = dark ? 'rgba(255,255,255,.12)' : 'rgba(0,0,0,.12)';
            var hover = dark ? 'rgba(10,132,255,.9)' : 'rgba(0,122,255,.9)';
            var sep   = dark ? 'rgba(255,255,255,.08)' : 'rgba(0,0,0,.08)';

            var host = document.createElement('div');
            host.style.cssText =
                'position:fixed;z-index:2147483647;left:0;top:0;' +
                'pointer-events:auto;';

            var shadow = host.attachShadow({ mode: 'closed' });
            var wrap = document.createElement('div');
            wrap.style.cssText =
                'position:fixed;background:' + bg + ';color:' + fg + ';' +
                'border:1px solid ' + bd + ';border-radius:8px;' +
                'box-shadow:0 10px 30px rgba(0,0,0,.22),0 2px 6px rgba(0,0,0,.10);' +
                'padding:5px;min-width:220px;max-width:320px;' +
                'font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",system-ui,sans-serif;' +
                'font-size:13px;line-height:1.3;user-select:none;' +
                '-webkit-user-select:none;';

            function addItem(label, onClick) {
                var row = document.createElement('div');
                row.textContent = label;
                row.style.cssText =
                    'padding:6px 10px;border-radius:5px;cursor:default;' +
                    'white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
                row.addEventListener('mouseenter', function () {
                    row.style.background = hover;
                    row.style.color = '#ffffff';
                });
                row.addEventListener('mouseleave', function () {
                    row.style.background = '';
                    row.style.color = '';
                });
                row.addEventListener('mousedown', function (e) {
                    // Use mousedown, not click — some pages stop click propagation
                    e.preventDefault(); e.stopPropagation();
                    closeMenu();
                    try { onClick(); } catch (_) {}
                });
                wrap.appendChild(row);
            }
            function addSep() {
                var s = document.createElement('div');
                s.style.cssText = 'height:1px;background:' + sep + ';margin:4px 6px;';
                wrap.appendChild(s);
            }

            addItem('Open Link in New Tab', function () {
                sendOpen('open_url_new_tab', href);
            });
            addItem('Open Link in New Window', function () {
                sendOpen('open_url_new_window', href);
            });
            addItem('Open Link in Private Window', function () {
                sendOpen('open_url_private_window', href);
            });
            addSep();
            addItem('Copy Link Address', function () {
                try { navigator.clipboard.writeText(href); }
                catch (_) {
                    var ta = document.createElement('textarea');
                    ta.value = href;
                    document.body.appendChild(ta);
                    ta.select();
                    try { document.execCommand('copy'); } catch (__) {}
                    ta.remove();
                }
            });

            shadow.appendChild(wrap);
            document.documentElement.appendChild(host);
            menuHost = host;

            // Clamp inside viewport
            var r = wrap.getBoundingClientRect();
            var left = Math.min(x, window.innerWidth - r.width - 8);
            var top  = Math.min(y, window.innerHeight - r.height - 8);
            wrap.style.left = Math.max(4, left) + 'px';
            wrap.style.top  = Math.max(4, top) + 'px';
        }

        document.addEventListener('contextmenu', function (e) {
            var a = getLink(e.target);
            if (!isWebLink(a)) return;          // let native menu handle non-links
            e.preventDefault();
            e.stopPropagation();
            showMenu(e.clientX, e.clientY, a.href);
        }, true);

        // Dismissal
        document.addEventListener('click', function () { closeMenu(); }, true);
        document.addEventListener('scroll', function () { closeMenu(); }, true);
        document.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') closeMenu();
        }, true);
        window.addEventListener('blur', closeMenu);
    })();

    // ==================================================================
    // 9. Open link modifier keys (Shift -> Window, Ctrl/Cmd/Middle -> Tab)
    // ==================================================================
    (function () {
        function getLink(el) {
            while (el && el !== document) {
                if (el.tagName === 'A') return el;
                el = el.parentNode;
            }
            return null;
        }
        function isWebLink(a) {
            return !!(a && a.href && /^https?:/i.test(a.href));
        }
        function hasNewWindowTarget(a) {
            var t = (a.getAttribute('target') || '').toLowerCase();
            return t === '_blank' || t === '_new';
        }
        function sendOpen(kind, href) {
            try {
                if (typeof sendAction === 'function') {
                    sendAction(kind + '?url=' + encodeURIComponent(href));
                } else {
                    fetch('http://shadhin.action/' + kind +
                          '?url=' + encodeURIComponent(href),
                          { mode: 'no-cors' });
                }
            } catch (_) {}
        }

        document.addEventListener('click', function (e) {
            if (e.button !== 0) return;
            var a = getLink(e.target);
            if (!isWebLink(a)) return;
            if (hasNewWindowTarget(a)) return;   // on_new_window handles it

            if (e.shiftKey) {
                e.preventDefault(); e.stopPropagation();
                sendOpen('open_url_new_window', a.href);
                return;
            }
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault(); e.stopPropagation();
                sendOpen('open_url_new_tab', a.href);
            }
        }, true);

        document.addEventListener('auxclick', function (e) {
            if (e.button !== 1) return;
            var a = getLink(e.target);
            if (!isWebLink(a)) return;
            if (hasNewWindowTarget(a)) return;
            e.preventDefault(); e.stopPropagation();
            sendOpen('open_url_new_tab', a.href);
        }, true);
    })();
})();
"#;

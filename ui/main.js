// Shadhin-Browser — Frontend UI Controller (Safari Chrome)

(function () {
    // Safe Tauri IPC Wrapper
    function invoke(cmd, args = {}) {
        if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
            return window.__TAURI__.core.invoke(cmd, args);
        } else if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
            return window.__TAURI_INTERNALS__.invoke(cmd, args);
        }
        return Promise.reject('Tauri IPC not available');
    }

    function listen(event, callback) {
        if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
            return window.__TAURI__.event.listen(event, callback);
        }
    }

    // State
    let tabs = [];
    let activeTabId = null;
    let pendingPasswordPrompt = null;
    let adBlockEnabled = true;

    // DOM Elements
    const tabListEl = document.getElementById('tab-list');
    const btnNewTab = document.getElementById('btn-new-tab');
    const tabScrollLeft = document.getElementById('tab-scroll-left');
    const tabScrollRight = document.getElementById('tab-scroll-right');
    const btnBack = document.getElementById('btn-back');
    const btnForward = document.getElementById('btn-forward');
    const btnBookmark = document.getElementById('btn-bookmark');
    const urlInput = document.getElementById('url-input');
    const btnReload = document.getElementById('btn-reload');
    const securityIcon = document.getElementById('security-icon');
    const loadingBar = document.getElementById('loading-bar');
    const btnShield = document.getElementById('btn-shield');
    const adblockBadge = document.getElementById('adblock-badge');
    const btnDownloads = document.getElementById('btn-downloads');
    const dlBadge = document.getElementById('dl-badge');
    const btnMenu = document.getElementById('btn-menu');

    // Traffic Lights
    const dotRed = document.getElementById('dot-red');
    const dotYellow = document.getElementById('dot-yellow');
    const dotGreen = document.getElementById('dot-green');

    // Popovers & Modals
    const downloadsPanel = document.getElementById('downloads-panel');
    const downloadsList = document.getElementById('downloads-list');
    const btnClearDownloads = document.getElementById('btn-clear-downloads');

    const menuPanel = document.getElementById('menu-panel');
    const menuNewTab = document.getElementById('menu-new-tab');
    const menuNewPrivate = document.getElementById('menu-new-private');
    const menuToggleAdblock = document.getElementById('menu-toggle-adblock');
    const menuAdblockCheck = document.getElementById('menu-adblock-check');
    const menuBookmarks = document.getElementById('menu-bookmarks');
    const menuPasswords = document.getElementById('menu-passwords');
    const menuSettings = document.getElementById('menu-settings');
    const menuClearData = document.getElementById('menu-clear-data');
    const menuAbout = document.getElementById('menu-about');

    const passwordBanner = document.getElementById('password-banner');
    const pwdOrigin = document.getElementById('pwd-origin');
    const btnPwdSave = document.getElementById('btn-pwd-save');
    const btnPwdDismiss = document.getElementById('btn-pwd-dismiss');
    const btnPwdNever = document.getElementById('btn-pwd-never');

    const settingsModal = document.getElementById('settings-modal');
    const btnCloseSettings = document.getElementById('btn-close-settings');
    const btnSaveSettings = document.getElementById('btn-save-settings');
    const vaultList = document.getElementById('vault-list');
    const btnClearNow = document.getElementById('btn-clear-now');

    const aboutModal = document.getElementById('about-modal');
    const btnCloseAbout = document.getElementById('btn-close-about');

    // Tracker Protection Popover
    const trackerPopover = document.getElementById('tracker-popover');
    const trackerToggleBtn = document.getElementById('tracker-toggle-btn');
    const trackerStateText = document.getElementById('tracker-state-text');
    const trackerShieldIcon = document.getElementById('tracker-shield-icon');

    // Custom Confirm Modal
    const confirmModal = document.getElementById('confirm-modal');
    const confirmMessage = document.getElementById('confirm-message');
    const confirmOk = document.getElementById('confirm-ok');
    const confirmCancel = document.getElementById('confirm-cancel');
    let _confirmCallback = null;

    function showConfirm(message, onConfirm) {
        confirmMessage.textContent = message;
        _confirmCallback = onConfirm;
        confirmModal.style.display = 'flex';
        expandToolbar(260);
    }

    confirmOk.addEventListener('click', () => {
        confirmModal.style.display = 'none';
        if (_confirmCallback) { _confirmCallback(); _confirmCallback = null; }
        checkAndCollapseToolbar();
    });

    confirmCancel.addEventListener('click', () => {
        confirmModal.style.display = 'none';
        _confirmCallback = null;
        checkAndCollapseToolbar();
    });

    // Toast notification
    function showToast(message, duration = 2500) {
        const existing = document.getElementById('shadhin-toast');
        if (existing) existing.remove();
        const toast = document.createElement('div');
        toast.id = 'shadhin-toast';
        toast.className = 'toast-notification';
        toast.textContent = message;
        document.body.appendChild(toast);
        setTimeout(() => { if (toast.parentNode) toast.remove(); }, duration);
    }

    // Dynamic Toolbar Height Expansion for Popovers and Overlays
    function expandToolbar(height = 480) {
        if (window.__TAURI__ || window.__TAURI_INTERNALS__) {
            invoke('set_toolbar_expanded', { expanded: true, height: height }).catch(() => {});
        }
    }

    function collapseToolbar() {
        if (window.__TAURI__ || window.__TAURI_INTERNALS__) {
            invoke('set_toolbar_expanded', { expanded: false, height: null }).catch(() => {});
        }
    }

    function isAnyOverlayOpen() {
        const isDownloads = downloadsPanel && downloadsPanel.style.display !== 'none';
        const isMenu = menuPanel && menuPanel.style.display !== 'none';
        const isTracker = trackerPopover && !trackerPopover.classList.contains('hidden');
        const isSettings = settingsModal && settingsModal.style.display !== 'none';
        const isAbout = aboutModal && aboutModal.style.display !== 'none';
        const isConfirm = confirmModal && confirmModal.style.display !== 'none';
        const isPwd = passwordBanner && passwordBanner.style.display !== 'none';
        return isDownloads || isMenu || isTracker || isSettings || isAbout || isConfirm || isPwd;
    }

    function checkAndCollapseToolbar() {
        if (!isAnyOverlayOpen()) {
            collapseToolbar();
        }
    }

    // ==========================================
    // Tab Management
    // ==========================================

    function renderTabs() {
        tabListEl.innerHTML = '';
        tabs.forEach(tab => {
            const tabEl = document.createElement('div');
            tabEl.className = 'tab-item' + (tab.id === activeTabId ? ' active' : '');
            tabEl.dataset.tabId = tab.id;

            // Favicon or default globe icon
            const iconEl = document.createElement('span');
            iconEl.className = 'tab-favicon';
            iconEl.innerHTML = tab.favicon ? `<img src="${tab.favicon}" width="14" height="14">` : '&#127760;';

            // Title
            const titleEl = document.createElement('span');
            titleEl.className = 'tab-title';
            titleEl.textContent = tab.title || 'New Tab';

            // Close button
            const closeBtn = document.createElement('button');
            closeBtn.className = 'tab-close-btn';
            closeBtn.innerHTML = '&times;';
            closeBtn.title = 'Close tab (Ctrl+W)';
            closeBtn.addEventListener('click', (e) => {
                e.stopPropagation();
                invoke('close_tab', { tabId: tab.id }).catch(console.error);
            });

            tabEl.appendChild(iconEl);
            tabEl.appendChild(titleEl);
            tabEl.appendChild(closeBtn);

            tabEl.addEventListener('click', () => {
                if (tab.id !== activeTabId) {
                    invoke('switch_tab', { tabId: tab.id }).catch(console.error);
                }
            });

            tabListEl.appendChild(tabEl);
        });

        // Update tab count label and scroll state after rendering
        updateTabScrollUI();
        // Auto-scroll active tab into view
        scrollActiveTabIntoView();
    }

    // ==========================================
    // Tab Scrolling
    // ==========================================

    const TAB_SCROLL_STEP = 180; // px per click

    function updateTabScrollUI() {
        // Show/hide scroll buttons based on overflow
        if (tabScrollLeft && tabScrollRight) {
            const canScroll = tabListEl.scrollWidth > tabListEl.clientWidth;
            tabScrollLeft.classList.toggle('hidden-btn', !canScroll || tabListEl.scrollLeft <= 0);
            tabScrollRight.classList.toggle('hidden-btn', !canScroll || tabListEl.scrollLeft >= tabListEl.scrollWidth - tabListEl.clientWidth - 2);
        }
    }

    function scrollActiveTabIntoView() {
        const activeEl = tabListEl.querySelector('.tab-item.active');
        if (activeEl) {
            activeEl.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
        }
        setTimeout(updateTabScrollUI, 320);
    }

    // Scroll left button: click scrolls left, double-click or shift+click jumps to 1st tab
    if (tabScrollLeft) {
        tabScrollLeft.title = "Scroll tabs left (Double-click: 1st tab)";
        tabScrollLeft.addEventListener('click', (e) => {
            e.stopPropagation();
            if (e.shiftKey) {
                tabListEl.scrollTo({ left: 0, behavior: 'smooth' });
            } else {
                tabListEl.scrollBy({ left: -TAB_SCROLL_STEP, behavior: 'smooth' });
            }
            setTimeout(updateTabScrollUI, 320);
        });
        tabScrollLeft.addEventListener('dblclick', (e) => {
            e.stopPropagation();
            tabListEl.scrollTo({ left: 0, behavior: 'smooth' });
            setTimeout(updateTabScrollUI, 320);
        });
    }

    // Scroll right button: click scrolls right, double-click or shift+click jumps to last tab
    if (tabScrollRight) {
        tabScrollRight.title = "Scroll tabs right (Double-click: last tab)";
        tabScrollRight.addEventListener('click', (e) => {
            e.stopPropagation();
            if (e.shiftKey) {
                tabListEl.scrollTo({ left: tabListEl.scrollWidth, behavior: 'smooth' });
            } else {
                tabListEl.scrollBy({ left: TAB_SCROLL_STEP, behavior: 'smooth' });
            }
            setTimeout(updateTabScrollUI, 320);
        });
        tabScrollRight.addEventListener('dblclick', (e) => {
            e.stopPropagation();
            tabListEl.scrollTo({ left: tabListEl.scrollWidth, behavior: 'smooth' });
            setTimeout(updateTabScrollUI, 320);
        });
    }

    // Update arrow visibility when tab list is scrolled with mouse wheel
    tabListEl.addEventListener('scroll', () => updateTabScrollUI());
    tabListEl.addEventListener('wheel', (e) => {
        e.preventDefault();
        tabListEl.scrollBy({ left: e.deltaY * 2, behavior: 'smooth' });
    }, { passive: false });

    function updateAddressBar() {
        const activeTab = tabs.find(t => t.id === activeTabId);
        if (!activeTab) return;

        if (document.activeElement !== urlInput) {
            urlInput.value = activeTab.url || '';
        }

        // Security indicator
        const isHttps = (activeTab.url || '').startsWith('https://');
        if (isHttps) {
            securityIcon.innerHTML = '&#128274;';
            securityIcon.title = 'Connection is secure (HTTPS)';
            securityIcon.style.color = '#34c759';
        } else if ((activeTab.url || '').startsWith('http://')) {
            securityIcon.innerHTML = '&#9888;';
            securityIcon.title = 'Connection is not secure (HTTP)';
            securityIcon.style.color = '#ff9500';
        } else {
            securityIcon.innerHTML = '&#128274;';
            securityIcon.title = 'Local or internal page';
            securityIcon.style.color = 'var(--text-secondary)';
        }

        // Loading state
        if (activeTab.is_loading) {
            loadingBar.classList.add('loading');
            btnReload.innerHTML = '&#10005;';
            btnReload.title = 'Stop loading (Esc)';
        } else {
            loadingBar.classList.remove('loading');
            btnReload.innerHTML = '&#8635;';
            btnReload.title = 'Reload (Ctrl+R)';
        }
    }

    // Refresh all tabs from backend
    function syncTabs() {
        invoke('get_tabs')
            .then(res => {
                tabs = res || [];
                return invoke('get_active_tab');
            })
            .then(active => {
                activeTabId = active ? active.id : (tabs[0] ? tabs[0].id : null);
                renderTabs();
                updateAddressBar();
            })
            .catch(console.error);
    }

    // ==========================================
    // Navigation & Address Bar
    // ==========================================

    urlInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            let raw = urlInput.value.trim();
            if (raw) {
                // Ctrl + Enter: automatically wrap with "www." and ".com"
                if (e.ctrlKey || e.metaKey) {
                    let domain = raw.replace(/^https?:\/\//i, '');
                    if (!domain.toLowerCase().startsWith('www.')) {
                        domain = 'www.' + domain;
                    }
                    if (!domain.toLowerCase().endsWith('.com')) {
                        domain = domain + '.com';
                    }
                    raw = 'https://' + domain;
                    urlInput.value = domain;
                }
                invoke('navigate_to', { url: raw }).catch(console.error);
                urlInput.blur();
            }
        } else if (e.key === 'Escape') {
            updateAddressBar();
            urlInput.blur();
        }
    });

    urlInput.addEventListener('focus', () => {
        urlInput.select();
    });

    btnReload.addEventListener('click', () => {
        const activeTab = tabs.find(t => t.id === activeTabId);
        if (activeTab && activeTab.is_loading) {
            invoke('stop').catch(console.error);
        } else {
            invoke('reload').catch(console.error);
        }
    });

    btnBack.addEventListener('click', () => {
        invoke('go_back').catch(console.error);
    });

    btnForward.addEventListener('click', () => {
        invoke('go_forward').catch(console.error);
    });

    btnNewTab.addEventListener('click', () => {
        invoke('create_tab', { url: null }).catch(console.error);
    });

    btnBookmark.addEventListener('click', () => {
        const activeTab = tabs.find(t => t.id === activeTabId);
        if (activeTab) {
            showToast(`Bookmarked: ${activeTab.title || activeTab.url}`);
        }
    });

    // ==========================================
    // Window Controls (Traffic Lights)
    // ==========================================

    dotRed.addEventListener('click', () => {
        invoke('window_close').catch(console.error);
    });

    dotYellow.addEventListener('click', () => {
        invoke('window_minimize').catch(console.error);
    });

    dotGreen.addEventListener('click', () => {
        invoke('window_maximize').catch(console.error);
    });

    // ==========================================
    // Window Dragging & Titlebar Double-Click
    // ==========================================

    document.addEventListener('mousedown', (e) => {
        // Only trigger on primary (left) mouse click
        if (e.button !== 0) return;

        // Skip if clicked on any interactive element
        const interactive = e.target.closest('button, input, select, textarea, .dot, .tab-close-btn, .tab-item, .popover-panel, .modal-dialog');
        if (interactive) return;

        // Check if clicked within a designated drag region
        if (e.target.closest('[data-tauri-drag-region]')) {
            invoke('window_start_dragging').catch(() => {
                if (window.__TAURI__ && window.__TAURI__.window && window.__TAURI__.window.getCurrentWindow) {
                    window.__TAURI__.window.getCurrentWindow().startDragging();
                }
            });
        }
    });

    // Double click header to toggle maximize / restore
    const safariHeader = document.querySelector('.safari-header');
    if (safariHeader) {
        safariHeader.addEventListener('dblclick', (e) => {
            if (e.target.closest('button, input, select, textarea, .dot')) return;
            invoke('window_maximize').catch(console.error);
        });
    }

    // ==========================================
    // Downloads Popover
    // ==========================================

    // Direct real-time callback invoked from Rust via tb.eval
    window.__onDownloadStarted = function(item) {
        if (dlBadge) dlBadge.style.display = 'block';
        const filename = (item && item.filename) || 'File';
        showToast('Download started: ' + filename);
        loadDownloads();
    };

    window.__onDownloadFinished = function(payload) {
        if (dlBadge) dlBadge.style.display = 'none';
        const filename = (payload && payload.filename) || 'File';
        if (payload && payload.success) {
            showToast('Download complete: ' + filename);
        } else {
            showToast('Download failed: ' + filename);
        }
        loadDownloads();
        // Automatically open downloads popover on completion so user sees the download list immediately
        if (downloadsPanel && downloadsPanel.style.display === 'none') {
            toggleDownloads();
        }
    };

    function toggleDownloads() {
        const isVisible = downloadsPanel.style.display !== 'none';
        closeAllPopovers();
        if (!isVisible) {
            loadDownloads();
            downloadsPanel.style.display = 'flex';
            dlBadge.style.display = 'none';
            expandToolbar(480);
        } else {
            checkAndCollapseToolbar();
        }
    }

    function loadDownloads() {
        invoke('get_downloads')
            .then(items => {
                downloadsList.innerHTML = '';
                if (!items || items.length === 0) {
                    downloadsList.innerHTML = '<div class="empty-state">No recent downloads</div>';
                    return;
                }

                items.forEach(item => {
                    const row = document.createElement('div');
                    row.className = 'download-row';

                    const header = document.createElement('div');
                    header.className = 'download-header';

                    const nameSpan = document.createElement('span');
                    nameSpan.className = 'download-filename';
                    nameSpan.textContent = item.filename;

                    const statusSpan = document.createElement('span');
                    statusSpan.className = 'download-status ' + item.status;
                    statusSpan.textContent = item.status === 'completed' ? '✓ Completed' : (item.status === 'downloading' ? '⏳ Downloading...' : item.status);

                    header.appendChild(nameSpan);
                    header.appendChild(statusSpan);

                    const actions = document.createElement('div');
                    actions.className = 'download-actions';

                    const btnOpen = document.createElement('button');
                    btnOpen.className = 'download-action-btn';
                    btnOpen.textContent = 'Open File';
                    btnOpen.addEventListener('click', () => {
                        invoke('open_download_file', { path: item.save_path }).catch(console.error);
                    });

                    const btnFolder = document.createElement('button');
                    btnFolder.className = 'download-action-btn';
                    btnFolder.textContent = 'Show in Folder';
                    btnFolder.addEventListener('click', () => {
                        invoke('open_download_folder', { path: item.save_path }).catch(console.error);
                    });

                    actions.appendChild(btnOpen);
                    actions.appendChild(btnFolder);

                    row.appendChild(header);
                    row.appendChild(actions);

                    downloadsList.appendChild(row);
                });
            })
            .catch(err => {
                console.error('Failed to load downloads:', err);
                downloadsList.innerHTML = '<div class="empty-state">No recent downloads</div>';
            });
    }

    btnDownloads.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleDownloads();
    });

    btnClearDownloads.addEventListener('click', () => {
        invoke('clear_downloads')
            .then(() => loadDownloads())
            .catch(console.error);
    });

    // ==========================================
    // Safari Main Menu
    // ==========================================

    function toggleMenu() {
        const isVisible = menuPanel.style.display !== 'none';
        closeAllPopovers();
        if (!isVisible) {
            menuPanel.style.display = 'block';
            expandToolbar(360);
        } else {
            checkAndCollapseToolbar();
        }
    }

    btnMenu.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleMenu();
    });

    menuNewTab.addEventListener('click', () => {
        closeAllPopovers();
        invoke('create_tab', { url: null }).catch(console.error);
    });

    menuNewPrivate.addEventListener('click', () => {
        closeAllPopovers();
        invoke('new_private_window').catch(console.error);
    });

    menuToggleAdblock.addEventListener('click', () => {
        invoke('toggle_adblock')
            .then(val => {
                adBlockEnabled = val;
                menuAdblockCheck.style.display = val ? 'inline' : 'none';
            })
            .catch(console.error);
    });

    menuBookmarks.addEventListener('click', () => {
        closeAllPopovers();
        showToast('Bookmarks: use the bookmark icon (🔖) in the toolbar to pin pages.');
    });

    menuPasswords.addEventListener('click', () => {
        closeAllPopovers();
        openSettings('passwords');
    });

    menuSettings.addEventListener('click', () => {
        closeAllPopovers();
        openSettings('general');
    });

    menuClearData.addEventListener('click', () => {
        closeAllPopovers();
        showConfirm('Clear browsing data (cache, session cookies, and history)?', () => {
            invoke('clear_browsing_data', {
                clearCache: true,
                clearCookies: true,
                clearHistory: true,
                clearPasswords: false
            }).then(() => showToast('Browsing data cleared.')).catch(console.error);
        });
    });

    menuAbout.addEventListener('click', () => {
        closeAllPopovers();
        aboutModal.style.display = 'flex';
        expandToolbar(450);
    });

    btnCloseAbout.addEventListener('click', () => {
        aboutModal.style.display = 'none';
        checkAndCollapseToolbar();
    });

    // ==========================================
    // Tracker Protection Popover
    // ==========================================

    function updateTrackerPopoverState(enabled) {
        trackerStateText.textContent = enabled ? 'Enabled' : 'Disabled';
        trackerToggleBtn.textContent = enabled ? 'Turn Off' : 'Turn On';
        trackerToggleBtn.className = 'tracker-toggle-btn ' + (enabled ? 'btn-on' : 'btn-off');
        trackerShieldIcon.style.color = enabled ? '#34c759' : '#ff3b30';
        // Update popover stats count
        const popCount = document.getElementById('tracker-popover-count');
        if (popCount) popCount.textContent = adblockBadge.textContent || '0';
        // Update badge / toolbar shield icon tint
        const shieldSpan = btnShield.querySelector('.shield-icon');
        if (shieldSpan) {
            shieldSpan.classList.toggle('shield-active', enabled);
            shieldSpan.classList.toggle('shield-disabled', !enabled);
        }
        menuAdblockCheck.style.display = enabled ? 'inline' : 'none';
    }

    // Shield button opens/closes the tracker popover
    btnShield.addEventListener('click', (e) => {
        e.stopPropagation();
        const isHidden = trackerPopover.classList.contains('hidden');
        closeAllPopovers();
        if (isHidden) {
            updateTrackerPopoverState(adBlockEnabled);
            updateBlockedCount();
            trackerPopover.classList.remove('hidden');
            expandToolbar(340);
        } else {
            checkAndCollapseToolbar();
        }
    });

    // Toggle button inside the popover calls toggle_ad_blocker with explicit enabled param
    trackerToggleBtn.addEventListener('click', () => {
        const newEnabled = !adBlockEnabled;
        invoke('toggle_ad_blocker', { enabled: newEnabled })
            .then(val => {
                adBlockEnabled = val;
                updateTrackerPopoverState(val);
                updateBlockedCount();
            })
            .catch(console.error);
    });

    // Direct real-time callback invoked from Rust via tb.eval
    window.__onTrackerBlocked = function(count) {
        const c = (typeof count === 'number') ? count : 0;
        if (adblockBadge) adblockBadge.textContent = c;
        const popCount = document.getElementById('tracker-popover-count');
        if (popCount) popCount.textContent = c;
    };

    function updateBlockedCount() {
        invoke('get_blocked_count')
            .then(count => {
                window.__onTrackerBlocked(count);
            })
            .catch(() => {});
    }
    setInterval(updateBlockedCount, 1000);

    // ==========================================
    // Password Manager Banner & Vault
    // ==========================================

    function showPasswordSavePrompt(data) {
        pendingPasswordPrompt = data;
        pwdOrigin.textContent = data.origin || 'this website';
        passwordBanner.style.display = 'flex';
        expandToolbar(220);
    }

    btnPwdSave.addEventListener('click', () => {
        if (pendingPasswordPrompt) {
            invoke('save_password', {
                origin: pendingPasswordPrompt.origin,
                username: pendingPasswordPrompt.username,
                password: pendingPasswordPrompt.password
            }).then(() => {
                passwordBanner.style.display = 'none';
                pendingPasswordPrompt = null;
                checkAndCollapseToolbar();
            }).catch(console.error);
        }
    });

    btnPwdDismiss.addEventListener('click', () => {
        passwordBanner.style.display = 'none';
        pendingPasswordPrompt = null;
        checkAndCollapseToolbar();
    });

    btnPwdNever.addEventListener('click', () => {
        passwordBanner.style.display = 'none';
        pendingPasswordPrompt = null;
        checkAndCollapseToolbar();
    });

    // ==========================================
    // Settings Modal
    // ==========================================

    function openSettings(section) {
        closeAllPopovers();
        settingsModal.style.display = 'flex';
        expandToolbar(null);

        invoke('get_settings').then(settings => {
            if (settings) {
                document.getElementById('setting-search-engine').value = settings.search_engine || 'duckduckgo';
                document.getElementById('setting-homepage').value = settings.homepage || '';
                document.getElementById('setting-download-dir').value = settings.download_dir || '';
                document.getElementById('setting-adblock').checked = settings.ad_block_enabled;
                document.getElementById('setting-cookies').checked = settings.block_third_party_cookies;
                document.getElementById('setting-dnt').checked = settings.do_not_track;
                document.getElementById('setting-clear-exit').checked = settings.clear_data_on_exit;
                document.getElementById('setting-pwd-manager').checked = settings.password_manager_enabled;
            }
        }).catch(console.error);

        loadVaultPasswords();
    }

    function loadVaultPasswords() {
        invoke('get_passwords').then(entries => {
            vaultList.innerHTML = '';
            if (!entries || entries.length === 0) {
                vaultList.innerHTML = '<div class="empty-state">No saved passwords in vault</div>';
                return;
            }

            entries.forEach(e => {
                const item = document.createElement('div');
                item.className = 'vault-item';

                const info = document.createElement('div');
                info.className = 'vault-info';
                info.innerHTML = `<strong>${e.origin}</strong><span>${e.username} &bull; ${e.password_preview}</span>`;

                const delBtn = document.createElement('button');
                delBtn.className = 'btn btn-secondary';
                delBtn.style.padding = '3px 8px';
                delBtn.style.fontSize = '11px';
                delBtn.textContent = 'Delete';
                delBtn.addEventListener('click', () => {
                    invoke('delete_password', { id: e.id })
                        .then(() => loadVaultPasswords())
                        .catch(console.error);
                });

                item.appendChild(info);
                item.appendChild(delBtn);
                vaultList.appendChild(item);
            });
        }).catch(console.error);
    }

    btnCloseSettings.addEventListener('click', () => {
        settingsModal.style.display = 'none';
        checkAndCollapseToolbar();
    });

    btnSaveSettings.addEventListener('click', () => {
        const updated = {
            search_engine: document.getElementById('setting-search-engine').value,
            homepage: document.getElementById('setting-homepage').value,
            download_dir: document.getElementById('setting-download-dir').value || null,
            ad_block_enabled: document.getElementById('setting-adblock').checked,
            block_third_party_cookies: document.getElementById('setting-cookies').checked,
            do_not_track: document.getElementById('setting-dnt').checked,
            clear_data_on_exit: document.getElementById('setting-clear-exit').checked,
            password_manager_enabled: document.getElementById('setting-pwd-manager').checked,
            dark_mode: 'system'
        };

        invoke('save_settings', { settings: updated })
            .then(() => {
                settingsModal.style.display = 'none';
                checkAndCollapseToolbar();
            })
            .catch(console.error);
    });

    btnClearNow.addEventListener('click', () => {
        showConfirm('Clear cache, cookies, and history now?', () => {
            invoke('clear_browsing_data', {
                clearCache: true,
                clearCookies: true,
                clearHistory: true,
                clearPasswords: false
            }).then(() => showToast('Browsing data cleared.')).catch(console.error);
        });
    });

    // Modal backdrop clicks
    settingsModal.addEventListener('click', (e) => {
        if (e.target === settingsModal) {
            settingsModal.style.display = 'none';
            checkAndCollapseToolbar();
        }
    });

    aboutModal.addEventListener('click', (e) => {
        if (e.target === aboutModal) {
            aboutModal.style.display = 'none';
            checkAndCollapseToolbar();
        }
    });

    confirmModal.addEventListener('click', (e) => {
        if (e.target === confirmModal) {
            confirmModal.style.display = 'none';
            _confirmCallback = null;
            checkAndCollapseToolbar();
        }
    });

    function closeAllPopovers() {
        downloadsPanel.style.display = 'none';
        menuPanel.style.display = 'none';
        if (trackerPopover) trackerPopover.classList.add('hidden');
        checkAndCollapseToolbar();
    }

    document.addEventListener('click', (e) => {
        let changed = false;
        if (!downloadsPanel.contains(e.target) && e.target !== btnDownloads && !btnDownloads.contains(e.target)) {
            if (downloadsPanel.style.display !== 'none') {
                downloadsPanel.style.display = 'none';
                changed = true;
            }
        }
        if (!menuPanel.contains(e.target) && e.target !== btnMenu && !btnMenu.contains(e.target)) {
            if (menuPanel.style.display !== 'none') {
                menuPanel.style.display = 'none';
                changed = true;
            }
        }
        if (trackerPopover && !trackerPopover.contains(e.target) && e.target !== btnShield && !btnShield.contains(e.target)) {
            if (!trackerPopover.classList.contains('hidden')) {
                trackerPopover.classList.add('hidden');
                changed = true;
            }
        }
        if (changed) {
            checkAndCollapseToolbar();
        }
    });

    // ==========================================
    // Tauri Backend Events
    // ==========================================

    if (window.__TAURI__ || window.__TAURI_INTERNALS__) {
        listen('tab-created', (e) => {
            syncTabs();
        });

        listen('tab-switched', (e) => {
            if (e.payload && e.payload.id) {
                activeTabId = e.payload.id;
                renderTabs();
                updateAddressBar();
            }
        });

        listen('tab-closed', (e) => {
            syncTabs();
        });

        listen('tab-title-changed', (e) => {
            const { id, title } = e.payload;
            const tab = tabs.find(t => t.id === id);
            if (tab) {
                tab.title = title;
                renderTabs();
            }
        });

        listen('tab-load-status', (e) => {
            const { id, isLoading, url } = e.payload;
            const tab = tabs.find(t => t.id === id);
            if (tab) {
                tab.is_loading = isLoading;
                if (url) tab.url = url;
                if (id === activeTabId) {
                    updateAddressBar();
                }
            }
        });

        listen('download-started', (e) => {
            if (window.__onDownloadStarted) {
                window.__onDownloadStarted(e.payload);
            }
        });

        listen('download-finished', (e) => {
            if (window.__onDownloadFinished) {
                window.__onDownloadFinished(e.payload);
            }
        });

        listen('tracker-blocked', (e) => {
            if (e.payload && typeof e.payload.count === 'number') {
                window.__onTrackerBlocked(e.payload.count);
            }
        });

        listen('password-save-prompt', (e) => {
            showPasswordSavePrompt(e.payload);
        });

        listen('focus-address-bar', () => {
            urlInput.focus();
            urlInput.select();
        });

        listen('trigger-shortcut', (e) => {
            const action = e.payload;
            if (action === 'switch-next-tab') {
                switchNextTab();
            } else if (action === 'switch-prev-tab') {
                switchPrevTab();
            } else if (action === 'switch-first-tab') {
                switchTabIndex(0);
            } else if (action === 'switch-last-tab') {
                switchTabIndex(tabs.length - 1);
            } else if (typeof action === 'string' && action.startsWith('switch_tab_')) {
                const n = parseInt(action.replace('switch_tab_', ''), 10) - 1;
                switchTabIndex(n);
            } else if (action === 'toggle-downloads') {
                toggleDownloads();
            } else if (action === 'open-settings') {
                openSettings('general');
            }
        });
    }

    function switchNextTab() {
        if (tabs.length < 2) return;
        const idx = tabs.findIndex(t => t.id === activeTabId);
        const next = tabs[(idx + 1) % tabs.length];
        invoke('switch_tab', { tabId: next.id }).catch(console.error);
    }

    function switchPrevTab() {
        if (tabs.length < 2) return;
        const idx = tabs.findIndex(t => t.id === activeTabId);
        const prev = tabs[(idx - 1 + tabs.length) % tabs.length];
        invoke('switch_tab', { tabId: prev.id }).catch(console.error);
    }

    function switchTabIndex(n) {
        if (tabs.length === 0) return;
        const target = n >= tabs.length ? tabs[tabs.length - 1] : tabs[n];
        if (target) invoke('switch_tab', { tabId: target.id }).catch(console.error);
    }

    // Keyboard Shortcuts (Window Level)
    window.addEventListener('keydown', (e) => {
        const isCtrl = e.ctrlKey || e.metaKey;
        const key = e.key;

        // Ctrl+T: New Tab
        if (isCtrl && !e.shiftKey && (key === 't' || key === 'T')) {
            e.preventDefault();
            invoke('create_tab', { url: null }).catch(console.error);
        }
        // Ctrl+W: Close Tab
        else if (isCtrl && !e.shiftKey && (key === 'w' || key === 'W')) {
            e.preventDefault();
            invoke('close_active_tab').catch(console.error);
        }
        // Ctrl+Shift+N: New Private Window
        else if (isCtrl && e.shiftKey && (key === 'n' || key === 'N')) {
            e.preventDefault();
            invoke('new_private_window').catch(console.error);
        }
        // Ctrl+L / Alt+D / F6: Focus Address Bar
        else if ((isCtrl && (key === 'l' || key === 'L')) || (e.altKey && (key === 'd' || key === 'D')) || key === 'F6') {
            e.preventDefault();
            urlInput.focus();
            urlInput.select();
        }
        // Ctrl+R / F5: Reload
        else if (((isCtrl && !e.shiftKey && (key === 'r' || key === 'R')) || key === 'F5') || (isCtrl && e.shiftKey && (key === 'r' || key === 'R'))) {
            e.preventDefault();
            invoke('reload').catch(console.error);
        }
        // Alt+Left: Back
        else if (e.altKey && key === 'ArrowLeft') {
            e.preventDefault();
            invoke('go_back').catch(console.error);
        }
        // Alt+Right: Forward
        else if (e.altKey && key === 'ArrowRight') {
            e.preventDefault();
            invoke('go_forward').catch(console.error);
        }
        // Ctrl+J: Downloads Popover
        else if (isCtrl && (key === 'j' || key === 'J')) {
            e.preventDefault();
            toggleDownloads();
        }
        // Ctrl+, / Ctrl+H: Settings
        else if ((isCtrl && key === ',') || (isCtrl && (key === 'h' || key === 'H'))) {
            e.preventDefault();
            openSettings('general');
        }
        // Ctrl+Tab / Ctrl+PageDown: Next Tab
        else if ((isCtrl && !e.shiftKey && key === 'Tab') || (isCtrl && key === 'PageDown')) {
            e.preventDefault();
            switchNextTab();
        }
        // Ctrl+Shift+Tab / Ctrl+PageUp: Previous Tab
        else if ((isCtrl && e.shiftKey && key === 'Tab') || (isCtrl && key === 'PageUp')) {
            e.preventDefault();
            switchPrevTab();
        }
        // Ctrl+1 .. Ctrl+8: Switch to tab 1..8
        else if (isCtrl && key >= '1' && key <= '8') {
            e.preventDefault();
            switchTabIndex(parseInt(key, 10) - 1);
        }
        // Ctrl+9: Switch to last tab
        else if (isCtrl && key === '9') {
            e.preventDefault();
            switchTabIndex(tabs.length - 1);
        }
        // Ctrl+Home: First tab
        else if (key === 'Home' && isCtrl) {
            e.preventDefault();
            switchTabIndex(0);
        }
        // Ctrl+End: Last tab
        else if (key === 'End' && isCtrl) {
            e.preventDefault();
            switchTabIndex(tabs.length - 1);
        }
        // Esc: Stop Loading / Close Popovers
        else if (key === 'Escape') {
            closeAllPopovers();
            invoke('stop').catch(console.error);
        }
    });

    // Initialize UI
    syncTabs();
    updateBlockedCount();
    // Sync adblock state from settings on startup
    invoke('get_settings').then(s => {
        if (s) {
            adBlockEnabled = !!s.ad_block_enabled;
            updateTrackerPopoverState(adBlockEnabled);
        }
    }).catch(() => {});
})();

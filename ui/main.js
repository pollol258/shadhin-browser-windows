// Shadhin-Browser — Frontend UI Controller (Safari Chrome)

// Early stub definition of direct host callbacks to prevent startup race with Rust tb.eval
window.__onDownloadStarted = window.__onDownloadStarted || function() {};
window.__onDownloadProgress = window.__onDownloadProgress || function() {};
window.__onDownloadStateChanged = window.__onDownloadStateChanged || function() {};
window.__onDownloadFinished = window.__onDownloadFinished || function() {};
window.__onTrackerBlocked = window.__onTrackerBlocked || function() {};

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
    const bookmarksPanel = document.getElementById('bookmarks-panel');
    const bookmarksList = document.getElementById('bookmarks-list');
    const btnAddCurrentBookmark = document.getElementById('btn-add-current-bookmark');

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
        const dlg = confirmModal.querySelector('.confirm-dialog') || confirmModal;
        measureAndExpand(dlg);
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
    let _mainWindowHeight = (window.screen && window.screen.availHeight) ? window.screen.availHeight : 800;
    let _currentExpandedHeight = null;
    let _popoverSyncDebounceTimer = null;

    function syncWindowSize() {
        invoke('get_window_size').then(size => {
            if (size && typeof size.height === 'number' && size.height > 100) {
                _mainWindowHeight = size.height;
                syncActiveOverlayHeight();
            }
        }).catch(() => {
            _mainWindowHeight = (window.screen && window.screen.availHeight) ? window.screen.availHeight : 800;
        });
    }

    function expandToolbar(height = 480) {
        if (window.__TAURI__ || window.__TAURI_INTERNALS__) {
            invoke('set_toolbar_expanded', { expanded: true, height: height }).catch(() => {});
        }
    }

    function collapseToolbar() {
        _currentExpandedHeight = null;
        if (window.__TAURI__ || window.__TAURI_INTERNALS__) {
            invoke('set_toolbar_expanded', { expanded: false, height: null }).catch(() => {});
        }
    }

    function getVisibleOverlays() {
        const list = [];
        if (settingsModal && settingsModal.style.display !== 'none') list.push({ el: settingsModal, full: true });
        if (aboutModal && aboutModal.style.display !== 'none') list.push({ el: aboutModal, full: true });
        if (confirmModal && confirmModal.style.display !== 'none') {
            const dlg = confirmModal.querySelector('.confirm-dialog') || confirmModal;
            list.push({ el: dlg, full: false });
        }
        if (downloadsPanel && downloadsPanel.style.display !== 'none') list.push({ el: downloadsPanel, full: false });
        if (bookmarksPanel && bookmarksPanel.style.display !== 'none' && !bookmarksPanel.classList.contains('hidden')) list.push({ el: bookmarksPanel, full: false });
        if (menuPanel && menuPanel.style.display !== 'none') list.push({ el: menuPanel, full: false });
        if (trackerPopover && !trackerPopover.classList.contains('hidden')) list.push({ el: trackerPopover, full: false });
        if (passwordBanner && passwordBanner.style.display !== 'none') list.push({ el: passwordBanner, full: false });
        return list;
    }

    function isAnyOverlayOpen() {
        return getVisibleOverlays().length > 0;
    }

    function getActiveOverlay() {
        const overlays = getVisibleOverlays();
        return overlays.length > 0 ? overlays[overlays.length - 1] : null;
    }

    function closeAllPopovers(exceptEl = null) {
        if (downloadsPanel && downloadsPanel !== exceptEl) {
            downloadsPanel.style.display = 'none';
        }
        if (menuPanel && menuPanel !== exceptEl) {
            menuPanel.style.display = 'none';
        }
        if (trackerPopover && trackerPopover !== exceptEl) {
            trackerPopover.classList.add('hidden');
        }
        if (bookmarksPanel && bookmarksPanel !== exceptEl) {
            bookmarksPanel.style.display = 'none';
            bookmarksPanel.classList.add('hidden');
        }
        if (passwordBanner && passwordBanner !== exceptEl) {
            passwordBanner.style.display = 'none';
        }
        if (!exceptEl) {
            checkAndCollapseToolbar();
        }
    }

    function isAnyPopoverOpen() {
        return (downloadsPanel && downloadsPanel.style.display !== 'none') ||
               (menuPanel && menuPanel.style.display !== 'none') ||
               (trackerPopover && !trackerPopover.classList.contains('hidden')) ||
               (bookmarksPanel && bookmarksPanel.style.display !== 'none' && !bookmarksPanel.classList.contains('hidden')) ||
               (passwordBanner && passwordBanner.style.display !== 'none');
    }

    // Expose global helper for outside-click and tab-side dismissal
    window.__closeAllPopovers = function() {
        closeAllPopovers();
    };

    function checkAndCollapseToolbar() {
        if (!isAnyOverlayOpen()) {
            collapseToolbar();
        }
    }

    function measureAndExpand(element) {
        if (!element) return;

        // Temporarily clear inline maxHeight to ensure scrollHeight reflects full rendered content
        element.style.maxHeight = 'none';

        requestAnimationFrame(() => {
            // Determine top offset
            let topOffset = 90;
            const rect = element.getBoundingClientRect();
            if (rect.top > 0) {
                topOffset = rect.top;
            } else if (element.offsetTop > 0) {
                topOffset = element.offsetTop;
            }

            // Cap max-height on the element to available window height minus offset and margin
            const availableHeight = Math.max(160, _mainWindowHeight - topOffset - 16);
            element.style.maxHeight = availableHeight + 'px';

            const contentHeight = Math.max(element.scrollHeight, element.offsetHeight);
            const targetHeight = Math.min(_mainWindowHeight, Math.ceil(topOffset + contentHeight + 16));

            if (_currentExpandedHeight === null || Math.abs(targetHeight - _currentExpandedHeight) > 2) {
                _currentExpandedHeight = targetHeight;
                expandToolbar(targetHeight);
            }
        });
    }

    function syncActiveOverlayHeight() {
        if (_popoverSyncDebounceTimer) {
            cancelAnimationFrame(_popoverSyncDebounceTimer);
        }
        _popoverSyncDebounceTimer = requestAnimationFrame(() => {
            _popoverSyncDebounceTimer = null;
            const overlays = getVisibleOverlays();
            if (overlays.length === 0) {
                checkAndCollapseToolbar();
                return;
            }
            if (overlays.some(o => o.full)) {
                if (_currentExpandedHeight !== 'full') {
                    _currentExpandedHeight = 'full';
                    expandToolbar(null);
                }
                return;
            }

            // Calculate maximum required height across all currently visible popovers
            let maxHeight = 82;
            overlays.forEach(item => {
                const el = item.el;
                let topOffset = 90;
                const rect = el.getBoundingClientRect();
                if (rect.top > 0) {
                    topOffset = rect.top;
                } else if (el.offsetTop > 0) {
                    topOffset = el.offsetTop;
                }

                const availableHeight = Math.max(160, _mainWindowHeight - topOffset - 16);
                el.style.maxHeight = availableHeight + 'px';

                const contentHeight = Math.max(el.scrollHeight, el.offsetHeight);
                const reqHeight = Math.min(_mainWindowHeight, Math.ceil(topOffset + contentHeight + 16));
                if (reqHeight > maxHeight) {
                    maxHeight = reqHeight;
                }
            });

            if (_currentExpandedHeight === null || Math.abs(maxHeight - _currentExpandedHeight) > 2) {
                _currentExpandedHeight = maxHeight;
                expandToolbar(maxHeight);
            }
        });
    }

    // Throttled ResizeObserver for active popovers
    if (window.ResizeObserver) {
        let _roThrottleTimer = null;
        const _resizeObserver = new ResizeObserver(() => {
            if (_roThrottleTimer) return;
            _roThrottleTimer = setTimeout(() => {
                _roThrottleTimer = null;
                syncActiveOverlayHeight();
            }, 50);
        });

        if (downloadsPanel) _resizeObserver.observe(downloadsPanel);
        if (bookmarksPanel) _resizeObserver.observe(bookmarksPanel);
        if (menuPanel) _resizeObserver.observe(menuPanel);
        if (trackerPopover) _resizeObserver.observe(trackerPopover);
        if (passwordBanner) _resizeObserver.observe(passwordBanner);
        const confirmDialog = document.querySelector('.confirm-dialog');
        if (confirmDialog) _resizeObserver.observe(confirmDialog);
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

        const tabUrl = activeTab.url || '';
        const isLocalNewTab = tabUrl === 'newtab.html' ||
                              tabUrl.includes('newtab.html') ||
                              tabUrl.startsWith('tauri://') ||
                              tabUrl.startsWith('http://tauri.localhost') ||
                              tabUrl.startsWith('https://tauri.localhost');

        if (document.activeElement !== urlInput) {
            if (isLocalNewTab) {
                urlInput.value = '';
                urlInput.placeholder = 'Search DuckDuckGo or enter URL';
            } else {
                urlInput.value = tabUrl;
                urlInput.placeholder = 'Search DuckDuckGo or enter URL';
            }
        }

        // Security indicator
        const isHttps = tabUrl.startsWith('https://');
        if (isHttps) {
            securityIcon.innerHTML = '&#128274;';
            securityIcon.title = 'Connection is secure (HTTPS)';
            securityIcon.style.color = '#34c759';
        } else if (tabUrl.startsWith('http://')) {
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

    let _isCreatingTab = false;
    async function handleCreateNewTab(targetUrl = null) {
        if (_isCreatingTab) return;
        _isCreatingTab = true;
        if (btnNewTab) {
            btnNewTab.style.pointerEvents = 'none';
            btnNewTab.style.opacity = '0.6';
        }
        try {
            await invoke('create_tab', { url: targetUrl });
        } catch (err) {
            console.error(err);
        } finally {
            setTimeout(() => {
                _isCreatingTab = false;
                if (btnNewTab) {
                    btnNewTab.style.pointerEvents = 'auto';
                    btnNewTab.style.opacity = '1';
                }
            }, 120);
        }
    }

    btnNewTab.addEventListener('click', () => {
        handleCreateNewTab(null);
    });

    btnBookmark.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleBookmarks();
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
    // Downloads Popover & Manager
    // ==========================================

    let _allDownloads = [];
    let _downloadSearchQuery = '';
    const dlRing = document.getElementById('dl-ring');
    const dlRingProgress = document.getElementById('dl-ring-progress');
    const downloadsSearchInput = document.getElementById('downloads-search-input');
    const btnClearSearch = document.getElementById('btn-clear-search');
    const btnOpenDownloadsFolder = document.getElementById('btn-open-downloads-folder');

    function formatBytes(bytes) {
        if (!bytes || bytes <= 0) return '0 B';
        const units = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(1024));
        const val = (bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1);
        return `${val} ${units[i]}`;
    }

    function formatSpeed(bps) {
        if (!bps || bps <= 0) return '0 B/s';
        return formatBytes(bps) + '/s';
    }

    function formatEta(seconds) {
        if (seconds === null || seconds === undefined || seconds < 0) return '';
        if (seconds < 60) return `${Math.round(seconds)} s left`;
        const mins = Math.floor(seconds / 60);
        const secs = Math.round(seconds % 60);
        if (mins < 60) return `${mins} m ${secs} s left`;
        const hours = Math.floor(mins / 60);
        return `${hours} h ${mins % 60} m left`;
    }

    function getFileCategory(filename) {
        if (!filename) return { icon: '&#128196;', type: 'doc', isExec: false };
        const ext = filename.split('.').pop().toLowerCase();
        if (['exe', 'msi', 'bat', 'cmd', 'ps1', 'scr', 'vbs', 'jar'].includes(ext)) {
            return { icon: '&#9881;', type: 'app', isExec: true };
        }
        if (['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'iso', 'dmg'].includes(ext)) {
            return { icon: '&#128230;', type: 'archive', isExec: false };
        }
        if (['mp4', 'mkv', 'avi', 'mov', 'webm', 'mp3', 'wav', 'flac', 'm4a', 'aac', 'ogg'].includes(ext)) {
            return { icon: '&#127916;', type: 'media', isExec: false };
        }
        if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'ico', 'bmp'].includes(ext)) {
            return { icon: '&#128444;', type: 'media', isExec: false };
        }
        if (['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt', 'rtf', 'csv'].includes(ext)) {
            return { icon: '&#128196;', type: 'doc', isExec: false };
        }
        return { icon: '&#128196;', type: 'doc', isExec: false };
    }

    function getDateGroup(dateStr) {
        if (!dateStr) return 'Earlier';
        const d = new Date(dateStr);
        if (isNaN(d.getTime())) return 'Earlier';
        const today = new Date();
        if (d.toDateString() === today.toDateString()) {
            return 'Today';
        }
        const yesterday = new Date();
        yesterday.setDate(yesterday.getDate() - 1);
        if (d.toDateString() === yesterday.toDateString()) {
            return 'Yesterday';
        }
        return 'Earlier';
    }

    function updateToolbarDownloadStatus() {
        const activeItems = _allDownloads.filter(i => i.state === 'in_progress' || i.state === 'paused');
        const count = activeItems.length;

        if (count > 0) {
            btnDownloads.classList.add('active');
            if (dlBadge) {
                dlBadge.style.display = 'flex';
                dlBadge.textContent = count > 9 ? '9+' : count;
            }

            let totalRecv = 0;
            let totalToRecv = 0;
            let hasKnownTotal = false;

            activeItems.forEach(item => {
                totalRecv += (item.received_bytes || 0);
                if (item.total_bytes && item.total_bytes > 0) {
                    totalToRecv += item.total_bytes;
                    hasKnownTotal = true;
                }
            });

            if (dlRingProgress) {
                const circumference = 87.96;
                if (hasKnownTotal && totalToRecv > 0) {
                    const ratio = Math.min(1.0, totalRecv / totalToRecv);
                    const offset = circumference * (1 - ratio);
                    dlRingProgress.style.strokeDashoffset = offset;
                } else {
                    dlRingProgress.style.strokeDashoffset = circumference * 0.75;
                }
            }
        } else {
            btnDownloads.classList.remove('active');
            if (dlBadge) {
                dlBadge.style.display = 'none';
            }
            if (dlRingProgress) {
                dlRingProgress.style.strokeDashoffset = 87.96;
            }
        }
    }

    function triggerDownloadPulse() {
        btnDownloads.classList.add('pulse');
        setTimeout(() => {
            btnDownloads.classList.remove('pulse');
        }, 600);
    }

    function renderDownloadsList() {
        if (!downloadsList) return;
        downloadsList.innerHTML = '';

        let items = Array.isArray(_allDownloads) ? _allDownloads : [];
        if (_downloadSearchQuery.trim()) {
            const q = _downloadSearchQuery.trim().toLowerCase();
            items = items.filter(i =>
                (i.filename && i.filename.toLowerCase().includes(q)) ||
                (i.source_domain && i.source_domain.toLowerCase().includes(q))
            );
        }

        if (items.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'empty-state';
            empty.textContent = _downloadSearchQuery.trim() ? 'No downloads match your search' : 'No recent downloads';
            downloadsList.appendChild(empty);
            if (downloadsPanel && downloadsPanel.style.display !== 'none') {
                syncActiveOverlayHeight();
            }
            return;
        }

        const activeList = items.filter(i => i.state === 'in_progress' || i.state === 'paused');
        const finishedList = items.filter(i => i.state !== 'in_progress' && i.state !== 'paused');

        if (activeList.length > 0) {
            const grpHeader = document.createElement('div');
            grpHeader.className = 'download-group-header';
            grpHeader.textContent = 'Active Downloads';
            downloadsList.appendChild(grpHeader);

            activeList.forEach(item => {
                try {
                    const rowEl = createDownloadRow(item);
                    if (rowEl) downloadsList.appendChild(rowEl);
                } catch (err) {
                    console.error('Error rendering active download row:', err);
                }
            });
        }

        const groups = { Today: [], Yesterday: [], Earlier: [] };
        finishedList.forEach(item => {
            const g = getDateGroup(item.finished_at || item.started_at);
            if (groups[g]) {
                groups[g].push(item);
            } else {
                groups.Earlier.push(item);
            }
        });

        ['Today', 'Yesterday', 'Earlier'].forEach(groupName => {
            const list = groups[groupName];
            if (list.length > 0) {
                const grpHeader = document.createElement('div');
                grpHeader.className = 'download-group-header';
                grpHeader.textContent = groupName;
                downloadsList.appendChild(grpHeader);

                list.forEach(item => {
                    try {
                        const rowEl = createDownloadRow(item);
                        if (rowEl) downloadsList.appendChild(rowEl);
                    } catch (err) {
                        console.error('Error rendering finished download row:', err);
                    }
                });
            }
        });

        if (downloadsPanel && downloadsPanel.style.display !== 'none') {
            syncActiveOverlayHeight();
        }
    }

    function createDownloadRow(item) {
        if (!item) return document.createElement('div');
        const filename = item.filename || 'download.bin';
        const cat = getFileCategory(filename);
        const state = item.state || 'in_progress';
        const row = document.createElement('div');
        row.className = `download-row ${state}`;
        row.setAttribute('data-download-id', item.id || '');

        const iconEl = document.createElement('div');
        iconEl.className = `download-file-icon ${cat.type}`;
        iconEl.innerHTML = cat.icon;

        const bodyEl = document.createElement('div');
        bodyEl.className = 'download-body';

        const titleRow = document.createElement('div');
        titleRow.className = 'download-header';

        const nameEl = document.createElement('span');
        nameEl.className = 'download-filename';
        nameEl.textContent = filename;
        nameEl.title = item.save_path || filename;
        nameEl.addEventListener('click', () => handleOpenFile(item));

        const domainEl = document.createElement('span');
        domainEl.className = 'download-domain';
        domainEl.textContent = item.source_domain || '';

        titleRow.appendChild(nameEl);
        titleRow.appendChild(domainEl);

        const infoEl = document.createElement('div');
        infoEl.className = 'download-info';

        const trackEl = document.createElement('div');
        trackEl.className = 'download-progress-track';

        const fillEl = document.createElement('div');
        fillEl.className = 'download-progress-fill';
        trackEl.appendChild(fillEl);

        const actionsEl = document.createElement('div');
        actionsEl.className = 'download-actions';

        bodyEl.appendChild(titleRow);
        bodyEl.appendChild(infoEl);
        bodyEl.appendChild(trackEl);
        bodyEl.appendChild(actionsEl);

        row.appendChild(iconEl);
        row.appendChild(bodyEl);

        updateRowDisplay(row, item, cat);

        if (item.state === 'completed') {
            invoke('check_download_file_exists', { id: item.id }).then(exists => {
                if (!exists) {
                    row.classList.add('file-deleted');
                    infoEl.textContent = 'File moved or deleted';
                    actionsEl.innerHTML = '';
                    actionsEl.appendChild(makeBtn('Retry', () => invoke('retry_download', { id: item.id }).catch(console.error), 'primary'));
                    actionsEl.appendChild(makeBtn('Remove', () => removeDownloadItem(item.id), 'danger'));
                }
            }).catch(console.error);
        }

        return row;
    }

    function handleOpenFile(item) {
        if (item.state !== 'completed') return;
        const cat = getFileCategory(item.filename);
        if (cat.isExec) {
            showConfirm(`"${item.filename}" is an executable application. Running files downloaded from the internet may harm your computer. Do you want to open it?`, () => {
                invoke('open_download_file', { path: item.save_path }).catch(console.error);
            });
        } else {
            invoke('open_download_file', { path: item.save_path }).catch(console.error);
        }
    }

    function makeBtn(label, onClick, extraClass) {
        const btn = document.createElement('button');
        btn.className = `download-action-btn ${extraClass || ''}`.trim();
        btn.textContent = label;
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            onClick();
        });
        return btn;
    }

    function removeDownloadItem(id) {
        invoke('remove_download', { id }).then(() => {
            _allDownloads = _allDownloads.filter(i => i.id !== id);
            renderDownloadsList();
            updateToolbarDownloadStatus();
        }).catch(console.error);
    }

    function updateRowDisplay(row, item, cat) {
        const infoEl = row.querySelector('.download-info');
        const trackEl = row.querySelector('.download-progress-track');
        const fillEl = row.querySelector('.download-progress-fill');
        const actionsEl = row.querySelector('.download-actions');

        if (!infoEl || !trackEl || !actionsEl) return;

        actionsEl.innerHTML = '';

        if (item.state === 'in_progress') {
            trackEl.style.display = 'block';
            const recv = formatBytes(item.received_bytes || 0);
            if (item.total_bytes && item.total_bytes > 0) {
                const total = formatBytes(item.total_bytes);
                const percent = Math.min(100, Math.round(((item.received_bytes || 0) / item.total_bytes) * 100));
                infoEl.textContent = `${recv} of ${total} (${percent}%)`;
                if (fillEl) fillEl.style.width = `${percent}%`;
            } else {
                infoEl.textContent = `${recv} downloaded`;
                if (fillEl) fillEl.style.width = '50%';
            }

            actionsEl.appendChild(makeBtn('Pause', () => invoke('pause_download', { id: item.id }).catch(console.error)));
            actionsEl.appendChild(makeBtn('Cancel', () => invoke('cancel_download', { id: item.id }).catch(console.error), 'danger'));
        } else if (item.state === 'paused') {
            trackEl.style.display = 'block';
            row.classList.add('paused');
            const recv = formatBytes(item.received_bytes || 0);
            const total = item.total_bytes ? ` of ${formatBytes(item.total_bytes)}` : '';
            infoEl.textContent = `Paused · ${recv}${total}`;

            actionsEl.appendChild(makeBtn('Resume', () => invoke('resume_download', { id: item.id }).catch(console.error), 'primary'));
            actionsEl.appendChild(makeBtn('Cancel', () => invoke('cancel_download', { id: item.id }).catch(console.error), 'danger'));
        } else if (item.state === 'completed') {
            trackEl.style.display = 'none';
            const size = formatBytes(item.received_bytes || item.total_bytes || 0);
            infoEl.textContent = `${size} · Download complete`;

            actionsEl.appendChild(makeBtn('Open File', () => handleOpenFile(item), 'primary'));
            actionsEl.appendChild(makeBtn('Show in Folder', () => invoke('open_download_folder', { path: item.save_path }).catch(console.error)));
            actionsEl.appendChild(makeBtn('Copy Link', () => {
                if (item.url) navigator.clipboard.writeText(item.url).then(() => showToast('Link copied to clipboard'));
            }));
            actionsEl.appendChild(makeBtn('Remove', () => removeDownloadItem(item.id)));
        } else {
            trackEl.style.display = 'none';
            infoEl.className = 'download-info error';
            const reason = item.error_reason || (item.state === 'cancelled' ? 'Cancelled' : 'Interrupted');
            infoEl.textContent = reason;

            actionsEl.appendChild(makeBtn('Retry', () => invoke('retry_download', { id: item.id }).catch(console.error), 'primary'));
            actionsEl.appendChild(makeBtn('Copy Link', () => {
                if (item.url) navigator.clipboard.writeText(item.url).then(() => showToast('Link copied to clipboard'));
            }));
            actionsEl.appendChild(makeBtn('Remove', () => removeDownloadItem(item.id)));
        }
    }

    // Direct real-time callback invoked from Rust via tb.eval
    window.__onDownloadProgress = function(payload) {
        if (!payload || !payload.id) return;

        let item = _allDownloads.find(i => i.id === payload.id);
        if (item) {
            item.received_bytes = payload.received_bytes;
            if (payload.total_bytes) item.total_bytes = payload.total_bytes;
            item.state = payload.state;
        } else {
            item = {
                id: payload.id,
                received_bytes: payload.received_bytes,
                total_bytes: payload.total_bytes,
                state: payload.state || 'in_progress',
                filename: 'Downloading...'
            };
            _allDownloads.unshift(item);
        }

        const row = document.querySelector(`[data-download-id="${payload.id}"]`);
        if (row) {
            const infoEl = row.querySelector('.download-info');
            const fillEl = row.querySelector('.download-progress-fill');
            const trackEl = row.querySelector('.download-progress-track');

            if (trackEl) trackEl.style.display = 'block';

            const recv = formatBytes(payload.received_bytes);
            let text = recv;
            if (payload.total_bytes && payload.total_bytes > 0) {
                const total = formatBytes(payload.total_bytes);
                const percent = Math.min(100, Math.round((payload.received_bytes / payload.total_bytes) * 100));
                text = `${recv} of ${total}`;
                if (payload.speed_bps > 0) text += ` · ${formatSpeed(payload.speed_bps)}`;
                if (payload.eta_seconds !== null && payload.eta_seconds !== undefined) {
                    const eta = formatEta(payload.eta_seconds);
                    if (eta) text += ` · ${eta}`;
                }
                if (fillEl) fillEl.style.width = `${percent}%`;
            } else {
                if (payload.speed_bps > 0) text += ` · ${formatSpeed(payload.speed_bps)}`;
                if (fillEl) fillEl.style.width = '40%';
            }

            if (infoEl) infoEl.textContent = text;
        }

        updateToolbarDownloadStatus();
    };

    window.__onDownloadStarted = function(item, autoOpen) {
        if (!item) return;
        const existingIdx = _allDownloads.findIndex(i => i.id === item.id);
        if (existingIdx >= 0) {
            _allDownloads[existingIdx] = item;
        } else {
            _allDownloads.unshift(item);
        }

        updateToolbarDownloadStatus();
        renderDownloadsList();

        showToast('Download started: ' + (item.filename || 'File'));

        if (autoOpen && downloadsPanel && downloadsPanel.style.display === 'none') {
            toggleDownloads();
        }
    };

    window.__onDownloadStateChanged = function(item, autoOpen, showNotif) {
        if (!item) return;
        const existingIdx = _allDownloads.findIndex(i => i.id === item.id);
        if (existingIdx >= 0) {
            _allDownloads[existingIdx] = item;
        } else {
            _allDownloads.unshift(item);
        }

        updateToolbarDownloadStatus();
        renderDownloadsList();

        if (item.state === 'completed') {
            triggerDownloadPulse();
            if (showNotif) {
                showToast('Download complete: ' + item.filename);
            }
        } else if (item.state === 'interrupted' || item.state === 'failed') {
            if (showNotif) {
                showToast('Download interrupted: ' + (item.error_reason || item.filename));
            }
        }

        if (autoOpen && downloadsPanel && downloadsPanel.style.display === 'none') {
            toggleDownloads();
        }
    };

    window.__onDownloadFinished = function(payload) {
        loadDownloads();
    };

    function toggleDownloads() {
        const isVisible = downloadsPanel.style.display !== 'none';
        if (isVisible) {
            downloadsPanel.style.display = 'none';
            checkAndCollapseToolbar();
        } else {
            closeAllPopovers(downloadsPanel);
            loadDownloads();
            downloadsPanel.style.display = 'flex';
            measureAndExpand(downloadsPanel);
        }
    }

    function loadDownloads() {
        invoke('get_downloads')
            .then(items => {
                _allDownloads = items || [];
                updateToolbarDownloadStatus();
                renderDownloadsList();
            })
            .catch(err => {
                console.error('Failed to load downloads:', err);
            });
    }

    btnDownloads.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleDownloads();
    });

    if (downloadsSearchInput) {
        downloadsSearchInput.addEventListener('input', (e) => {
            _downloadSearchQuery = e.target.value;
            if (btnClearSearch) {
                btnClearSearch.style.display = _downloadSearchQuery ? 'block' : 'none';
            }
            renderDownloadsList();
        });
    }

    if (btnClearSearch) {
        btnClearSearch.addEventListener('click', () => {
            downloadsSearchInput.value = '';
            _downloadSearchQuery = '';
            btnClearSearch.style.display = 'none';
            renderDownloadsList();
        });
    }

    if (btnOpenDownloadsFolder) {
        btnOpenDownloadsFolder.addEventListener('click', () => {
            invoke('open_downloads_folder').catch(console.error);
        });
    }

    btnClearDownloads.addEventListener('click', () => {
        showConfirm('Clear all completed and interrupted downloads from the list?', () => {
            invoke('clear_downloads')
                .then(() => loadDownloads())
                .catch(console.error);
        });
    });

    // ==========================================
    // Safari Main Menu
    // ==========================================

    function toggleMenu() {
        const isVisible = menuPanel.style.display !== 'none';
        if (isVisible) {
            menuPanel.style.display = 'none';
            checkAndCollapseToolbar();
        } else {
            closeAllPopovers(menuPanel);
            menuPanel.style.display = 'block';
            measureAndExpand(menuPanel);
        }
    }

    btnMenu.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleMenu();
    });

    menuNewTab.addEventListener('click', () => {
        closeAllPopovers();
        handleCreateNewTab(null);
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

    menuBookmarks.addEventListener('click', (e) => {
        e.stopPropagation();
        closeAllPopovers();
        toggleBookmarks();
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
        _currentExpandedHeight = 'full';
        expandToolbar(null);
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
        if (popCount) {
            popCount.textContent = (adblockBadge && adblockBadge.textContent) ? adblockBadge.textContent : '0';
        }
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

        if (isHidden) {
            closeAllPopovers(trackerPopover);
            updateTrackerPopoverState(adBlockEnabled);
            updateBlockedCount();
            trackerPopover.classList.remove('hidden');
            measureAndExpand(trackerPopover);
        } else {
            trackerPopover.classList.add('hidden');
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
        const c = (typeof count === 'number') ? count : (parseInt(count, 10) || 0);
        if (adblockBadge) {
            adblockBadge.textContent = c > 999 ? '999+' : c;
            adblockBadge.style.display = c > 0 ? 'inline-flex' : 'none';
            adblockBadge.classList.remove('hidden');
        }
        const popCount = document.getElementById('tracker-popover-count');
        if (popCount) {
            popCount.textContent = c;
        }
    };

    function updateBlockedCount() {
        invoke('get_blocked_count')
            .then(count => {
                if (window.__onTrackerBlocked) {
                    window.__onTrackerBlocked(count);
                }
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
        measureAndExpand(passwordBanner);
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
    // Bookmarks Popover & Manager
    // ==========================================

    function toggleBookmarks() {
        if (!bookmarksPanel) return;
        const isOpen = bookmarksPanel.style.display !== 'none' && !bookmarksPanel.classList.contains('hidden');
        if (isOpen) {
            bookmarksPanel.style.display = 'none';
            bookmarksPanel.classList.add('hidden');
            checkAndCollapseToolbar();
        } else {
            closeAllPopovers(bookmarksPanel);
            bookmarksPanel.classList.remove('hidden');
            bookmarksPanel.style.display = 'flex';
            loadBookmarks();
            measureAndExpand(bookmarksPanel);
        }
    }

    function loadBookmarks() {
        invoke('get_bookmarks')
            .then(items => {
                renderBookmarks(items || []);
            })
            .catch(err => {
                console.error('Failed to load bookmarks:', err);
                renderBookmarks([]);
            });
    }

    function renderBookmarks(items) {
        if (!bookmarksList) return;
        bookmarksList.innerHTML = '';

        if (!items || items.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'empty-state';
            empty.id = 'bookmarks-empty-state';
            empty.textContent = 'No bookmarks saved yet';
            bookmarksList.appendChild(empty);
            if (bookmarksPanel && bookmarksPanel.style.display !== 'none' && !bookmarksPanel.classList.contains('hidden')) {
                measureAndExpand(bookmarksPanel);
            }
            return;
        }

        items.forEach(item => {
            const row = document.createElement('div');
            row.className = 'bookmark-row';
            row.title = item.url || '';

            const icon = document.createElement('span');
            icon.className = 'bookmark-icon';
            icon.textContent = '🔖';

            const info = document.createElement('div');
            info.className = 'bookmark-info';

            const title = document.createElement('div');
            title.className = 'bookmark-title';
            title.textContent = item.title || item.url || 'Untitled';

            const url = document.createElement('div');
            url.className = 'bookmark-url';
            url.textContent = item.url || '';

            info.appendChild(title);
            info.appendChild(url);

            const delBtn = document.createElement('button');
            delBtn.className = 'btn-delete-bookmark';
            delBtn.innerHTML = '&times;';
            delBtn.title = 'Remove bookmark';
            delBtn.addEventListener('click', (e) => {
                e.stopPropagation();
                invoke('delete_bookmark', { id: item.id })
                    .then(() => {
                        showToast('Bookmark removed');
                        loadBookmarks();
                    })
                    .catch(console.error);
            });

            row.appendChild(icon);
            row.appendChild(info);
            row.appendChild(delBtn);

            row.addEventListener('click', (e) => {
                if (e.target.closest('.btn-delete-bookmark')) return;
                if (item.url) {
                    const cleanUrl = (item.url.startsWith('http://') || item.url.startsWith('https://'))
                        ? item.url
                        : 'https://' + item.url;
                    invoke('navigate_to', { url: cleanUrl }).catch(console.error);
                    urlInput.value = item.url;
                }
                bookmarksPanel.style.display = 'none';
                bookmarksPanel.classList.add('hidden');
                checkAndCollapseToolbar();
            });

            bookmarksList.appendChild(row);
        });

        if (bookmarksPanel && bookmarksPanel.style.display !== 'none' && !bookmarksPanel.classList.contains('hidden')) {
            measureAndExpand(bookmarksPanel);
        }
    }

    if (btnAddCurrentBookmark) {
        btnAddCurrentBookmark.addEventListener('click', (e) => {
            e.stopPropagation();
            const activeTab = tabs.find(t => t.id === activeTabId);
            if (!activeTab || !activeTab.url) {
                showToast('No active tab to bookmark');
                return;
            }
            const title = activeTab.title && activeTab.title !== 'New Tab' ? activeTab.title : activeTab.url;
            invoke('save_bookmark', { title: title, url: activeTab.url })
                .then(() => {
                    showToast(`Bookmarked: ${title}`);
                    loadBookmarks();
                })
                .catch(console.error);
        });
    }

    // ==========================================
    // Settings Modal
    // ==========================================

    function openSettings(section) {
        closeAllPopovers();
        settingsModal.style.display = 'flex';
        _currentExpandedHeight = 'full';
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
                document.getElementById('setting-ask-location').checked = !!settings.ask_download_location;
                document.getElementById('setting-auto-open-downloads').checked = settings.auto_open_downloads_panel !== false;
                document.getElementById('setting-download-notification').checked = settings.show_download_notification !== false;
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
            ask_download_location: document.getElementById('setting-ask-location').checked,
            auto_open_downloads_panel: document.getElementById('setting-auto-open-downloads').checked,
            show_download_notification: document.getElementById('setting-download-notification').checked,
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

    const btnBrowseDownloadDir = document.getElementById('btn-browse-download-dir');
    if (btnBrowseDownloadDir) {
        btnBrowseDownloadDir.addEventListener('click', () => {
            invoke('pick_download_folder').then(folder => {
                if (folder) {
                    document.getElementById('setting-download-dir').value = folder;
                }
            }).catch(console.error);
        });
    }

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

    document.addEventListener('click', (e) => {
        if (!isAnyPopoverOpen()) return;
        if (e.target.closest('.popover-panel, button, [role="button"], .safari-header, .tab-strip')) {
            return;
        }
        window.__closeAllPopovers();
    });

    // ==========================================
    // Tauri Backend Events
    // ==========================================

    if (window.__TAURI__ || window.__TAURI_INTERNALS__) {
        // Handles target="_blank" and window.open from any tab webview
        listen('open-url-in-new-tab', async (event) => {
            const url = event.payload;
            if (!url || typeof url !== 'string') return;
            try {
                await invoke('create_tab', { url });
            } catch (e) {
                console.error('[open-url-in-new-tab] create_tab failed:', e);
            }
        });

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
                window.__onDownloadStarted(e.payload, false);
            }
        });

        listen('download-progress', (e) => {
            if (window.__onDownloadProgress) {
                window.__onDownloadProgress(e.payload);
            }
        });

        listen('download-state-changed', (e) => {
            if (window.__onDownloadStateChanged) {
                window.__onDownloadStateChanged(e.payload, false, true);
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
            } else if (action === 'toggle-bookmarks') {
                toggleBookmarks();
            } else if (action === 'open-settings') {
                openSettings('general');
            }
        });

        listen('window-resized', (e) => {
            if (e.payload && typeof e.payload.height === 'number') {
                _mainWindowHeight = e.payload.height;
                syncActiveOverlayHeight();
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
            handleCreateNewTab(null);
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
        // Ctrl+B: Bookmarks Popover
        else if (isCtrl && !e.shiftKey && (key === 'b' || key === 'B')) {
            e.preventDefault();
            toggleBookmarks();
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
    syncWindowSize();
    syncTabs();
    updateBlockedCount();
    document.addEventListener('DOMContentLoaded', updateBlockedCount);
    loadDownloads();
    loadBookmarks();
    document.addEventListener('DOMContentLoaded', loadBookmarks);
    // Sync adblock state from settings on startup
    invoke('get_settings').then(s => {
        if (s) {
            adBlockEnabled = !!s.ad_block_enabled;
            updateTrackerPopoverState(adBlockEnabled);
        }
    }).catch(() => {});
})();

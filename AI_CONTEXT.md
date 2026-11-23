# Shadhin-Browser (স্বাধীন ব্রাউজার) — AI Architecture & Technical Context

> **Target Audience:** Any AI Assistant, LLM Agent, or Senior Systems Engineer inspecting, maintaining, or extending this codebase.  
> **Repository Location:** `D:\MyProject\pollol258`  
> **Remote Repository:** `https://github.com/pollol258/shadhin-browser-windows.git`  
> **Current Version:** `1.1.0`  
> **Tech Stack:** Tauri 2.12.5 (Rust) + Microsoft Edge WebView2 (Windows 10/11) + Vanilla Modern ES6 JavaScript / HTML5 / CSS3 (Zero node runtime / zero npm build dependencies for frontend).

---

## 1. Executive Summary & Purpose

**Shadhin-Browser** is a fast, memory-efficient, privacy-focused desktop web browser for Windows. It combines an Apple Safari macOS-inspired aesthetic with a multi-webview child architecture powered by Microsoft Edge WebView2 and Rust.

### Key Architectural Pillars
- **Zero Heavy Frontend Frameworks:** The UI is completely vanilla (HTML5, CSS3, ES6 JS). No React, Vue, Svelte, or Webpack bundle overhead. Fast cold start and minimal memory footprint.
- **Hardware/OS-Level DPAPI Encryption:** Saved website passwords are encrypted directly through the Windows Data Protection API (`CryptProtectData` / `CryptUnprotectData`), securing them against cross-account or cross-machine extraction.
- **Dual-Layer Tracker & Ad Protection:** Combines a Rust-level network interceptor (`on_web_resource_request` with EasyList-lite and telemetry path blocking) and an in-page DOM cosmetic blocker via `PAGE_INJECTION_SCRIPT` with `MutationObserver`.
- **Dynamic Multi-Webview Bounds Management:** Overcomes child webview clipping in Tauri 2 by dynamically resizing the top chrome webview when popovers or modals open, while maintaining transparency over underlying web content.
- **Dual-Layer Keyboard Shortcut Interception:** Injects a global shortcut capture listener into content pages (`http://shadhin.action/<action>`), bypassing cross-origin iframe and Content Security Policy (CSP) restrictions.
- **Non-Blocking STA COM Download Pipeline:** Multi-threaded download tracking using native WebView2 COM hooks with asynchronous event dispatching, guaranteeing zero UI freezing (`Not Responding`) even on heavy multithreaded downloads.

---

## 2. Architecture & Multi-Webview Design

Shadhin-Browser uses Tauri 2's **Multi-Webview Child Architecture**:

```text
+-----------------------------------------------------------------------------------+
| Main Frameless Window ("main") — decorations: false, resizable: true              |
|                                                                                   |
| +-------------------------------------------------------------------------------+ |
| | Toolbar Child Webview ("toolbar") — Height: 82.0px (Default) / Dynamic Expand | |
| | Row 1 (44px): [Traffic Lights] [Back/Fwd/Reload] [Address Bar] [Shield] [DL] [Menu]|
| | Row 2 (36px): [◀] [Tab 1] [Tab 2] ... [Tab N] [+] [▶]                         | |
| +-------------------------------------------------------------------------------+ |
|                                                                                   |
| +-------------------------------------------------------------------------------+ |
| | Active Tab Child Webview ("tab_<uuid>") — Position: (0, 82), Size: (W, H - 82)| |
| | Renders the live web page inside isolated WebView2 engine                      | |
| +-------------------------------------------------------------------------------+ |
+-----------------------------------------------------------------------------------+
```

### 2.1. The Multi-Webview Bounds Constraint & Dynamic Expansion Fix
- **The Constraint:** In Tauri 2 / WebView2, a child webview physically clips all HTML content outside its bounding rectangle. The chrome toolbar webview is physically positioned at `(0.0, 0.0)` with a default height of `82.0px` (`TOOLBAR_HEIGHT`). Any popover or modal rendered at `top: 90px` or full screen is clipped and 100% invisible if the toolbar webview remains at 82px.
- **The Solution (Dynamic Toolbar Expansion & Content Measurement):**
  1. The toolbar webview is instantiated with `.transparent(true)` in `src-tauri/src/lib.rs`, and `html, body` has `background: transparent;`. On expansion, the Win32 HWND of the toolbar is raised with `HWND_TOP` to guarantee popover clicks are not stolen by the underlying tab.
  2. **Dynamic Measurement (No Hard-Coded Heights):** When any popover or overlay opens (Safari menu, downloads panel, tracker protection popover, confirm dialog, password save banner), `measureAndExpand(element)` measures the real rendered content height (`topOffset + scrollHeight + 16px bottom margin`) via `requestAnimationFrame` after layout.
  3. **Window Height Capping & Thin Scrollbars:** Popovers are capped at `Math.max(120, _mainWindowHeight - topOffset - 16)`. If content is taller than available window height, `overflow-y: auto` with custom styled thin scrollbars (`5px`, theme-aware) ensures all items remain reachable.
  4. **Dynamic Resizing with Progress Tick Immunity:** A throttled `ResizeObserver` (50ms) and `syncActiveOverlayHeight()` track dynamic changes (e.g. downloads list growing, shrinking, or filtered by search) and update the toolbar height only when the measured target height changes by > 2px. Rapid download progress ticks (~5Hz) do not alter row height and thus incur zero IPC overhead or flicker.
  5. Full-window overlays (Settings modal, About modal) call `expandToolbar(null)` to expand to 100% of the window height with a translucent backdrop.
  6. When all popovers/modals close (via Esc, outside click, or toggle), `collapseToolbar()` immediately resets the toolbar height back to `TOOLBAR_HEIGHT = 82.0px`.

### 2.2. Multi-Window & Popups ("Open in New Window")
- When opening a link in a new window (via context menu or `Shift+Click`), the backend helper `spawn_browser_window` in `src-tauri/src/commands.rs` is used.
- **CRITICAL PATTERN:**
  - Must use `tauri::WebviewWindowBuilder::new(...)` (which atomically creates the OS window and the WebView2 engine together), NOT `WindowBuilder` + `add_child` (which causes a blank white window race condition in Tauri 2).
  - Uses `decorations(true)` for secondary windows so standard OS minimize/maximize/close buttons and title bar drag areas work out of the box.
  - **Unique Data Directory:** WebView2 does not allow two running processes/windows to lock the same user data directory simultaneously. Secondary windows use `profiles/window_<uuid>`, private windows use `profiles/incognito_<uuid>`, and the main window uses `profiles/default`.

---

## 3. Directory & File Structure

```text
D:\MyProject\pollol258/
├── src-tauri/                          # Rust Backend Core
│   ├── src/
│   │   ├── main.rs                    # Application entry point (calls shadhin_browser_lib::run())
│   │   ├── lib.rs                     # Window setup, toolbar builder, resize listeners, command registration
│   │   ├── commands.rs                # All Tauri IPC commands, shortcut dispatchers, spawn_browser_window
│   │   ├── blocker.rs                 # EasyList-lite network blocker & atomic counter
│   │   ├── downloads.rs               # Non-blocking COM download manager & persistent JSON history
│   │   ├── passwords.rs               # Windows DPAPI hardware-bound password vault
│   │   ├── settings.rs                # User settings (search engine, homepage, download dir, ask location, etc.)
│   │   ├── fullscreen.rs              # HTML5 fullscreen state tracking
│   │   └── browser/
│   │       ├── mod.rs                 # PAGE_INJECTION_SCRIPT & toolbar height constants
│   │       ├── navigation.rs          # URL normalization & search engine query formatting
│   │       └── tabs.rs                # TabManager data structures & active tab state
│   ├── capabilities/
│   │   └── default.json               # Tauri 2 security permissions & capabilities
│   ├── icons/                         # Multi-resolution icons (.ico, .png, .icns, AppX assets)
│   ├── tauri.conf.json                # Tauri 2 configuration (productName, version, bundle, webviewInstallMode)
│   └── Cargo.toml                     # Rust dependencies (tauri 2.12.5, webview2-com, windows 0.58, serde, etc.)
├── ui/                                # Frontend UI (Zero Node build dependencies)
│   ├── index.html                     # Safari layout (traffic lights, address bar, popovers, modals)
│   ├── styles.css                     # Apple Safari macOS theme (light/dark mode, acrylic blur, drag regions)
│   └── main.js                        # UI controller, IPC caller, keyboard listener, tab strip renderer
├── .gitignore                         # Build artifact ignore list (target/, dist/, etc.)
├── README.md                          # Human-facing project overview & download links
└── AI_CONTEXT.md                      # This file (AI and developer architecture manual)
```

---

## 4. Key Subsystems & Implementation Details

### 4.1. Tab Management (`tabs.rs`, `commands.rs`, `main.js`)
- **Backend:** `TabManager` holds a `Vec<TabInfo>` and `active_tab_id: Option<String>`.
- **Lifecycle:**
  - `create_tab(url)`: Rate-limited with 120ms cooldown guard (`last_create_tab`). Hides the currently active tab webview, registers `TabInfo` into `TabManager` *before* child webview creation, instantiates the child webview, re-raises the toolbar HWND (`HWND_TOP`) so toolbar clicks are never eaten by the new child webview, and defers native WebView2 hooks (`hook_tab_download`, `hook_tab_blocker`, `hook_tab_fullscreen`) to the tab's first `PageLoadEvent::Started` event via an atomic guard. Emits `"tab-created"` and `"tab-switched"`.
  - `switch_tab(tab_id)`: Resizes target tab, calls `wv.show()`, and hides all other tabs.
  - `close_tab(tab_id)`: Destroys webview, switches to adjacent tab. If empty, automatically spawns a fresh homepage tab.
- **Frontend Tab Strip:**
  - Located on Row 2 (36px high).
  - Single-clicking `◀` or `▶` scrolls horizontally. Double-clicking jumps directly to Tab 1 (`scrollTo({ left: 0 })`).
  - Supports mouse-wheel horizontal scrolling.

### 4.2. Window Dragging & Window Controls
- **Frameless Setup:** `decorations: false`, `resizable: true`, `shadow: true`.
- **Dragging:** Uses Tauri capability `"core:window:allow-start-dragging"`. `main.js` catches `mousedown` with `e.button === 0` on elements with `[data-tauri-drag-region]`, ignoring interactive controls (`button, input, select, textarea, .dot, .tab-item, .popover-panel`).
- **Header Double-Click:** Double-clicking the empty space on `.safari-header` toggles maximize / restore via `window_maximize`.
- **Traffic Light Dots:**
  - Red: `window_close`
  - Yellow: `window_minimize`
  - Green: `window_maximize` (toggles maximize/restore)

### 4.3. Dual-Layer Keyboard Shortcuts & Link Actions
Shortcuts must work even when keyboard focus is inside a third-party webpage or foreign iframe.
1. **In-Page Content Script (`PAGE_INJECTION_SCRIPT` in `browser/mod.rs`):**
   - Attached to every tab on initialization.
   - Listens on `window` in the capture phase (`keydown`).
   - Forwards shortcuts to the host via `http://shadhin.action/<action>` (or `window.__TAURI_INTERNALS__.invoke`).
   - Rust intercepts `http://shadhin.action/` in `on_web_resource_request`, responds with `204 NO_CONTENT`, and dispatches via `handle_injected_action()`.
2. **Main Toolbar Layer (`main.js`):**
   - Listens on the toolbar window for global hotkeys when the address bar or chrome is focused.
3. **Supported Shortcuts:**
   - `Ctrl + T`: New tab
   - `Ctrl + W`: Close active tab
   - `Ctrl + R` / `F5`: Reload
   - `Ctrl + Shift + R` / `Ctrl + F5`: Hard reload
   - `Ctrl + L` / `Alt + D` / `F6`: Focus address bar (`focus_address_bar` -> `tb.set_focus()` + select all)
   - `Alt + Left`: Back
   - `Alt + Right`: Forward
   - `Ctrl + Shift + N`: New private window
   - `Ctrl + Tab` / `Ctrl + PageDown`: Next tab
   - `Ctrl + Shift + Tab` / `Ctrl + PageUp`: Previous tab
   - `Ctrl + 1` .. `Ctrl + 8`: Switch to tab 1 through 8
   - `Ctrl + 9`: Switch to last tab
   - `Ctrl + Home`: First tab
   - `Ctrl + End`: Last tab
   - `Ctrl + J`: Toggle downloads panel
   - `Ctrl + ,`: Open settings
   - `Ctrl + B`: Bookmarks toast
   - `Esc`: Stop loading / close all active popovers
4. **In-Page Link Context Menu & Modifier Navigation:**
   - Intercepts link right-clicks (`contextmenu` on `<a>`) with a theme-aware Shadow-DOM Safari-style context menu:
     - *Open Link in New Tab*
     - *Open Link in New Window*
     - *Open Link in Private Window*
     - *Copy Link Address*
   - Modifier shortcuts:
     - `Ctrl/Cmd + Click` and middle-click (`auxclick` button 1) open link in new tab.
     - `Shift + Click` opens link in a new window.
     - `target="_blank"` links route to new tabs via `on_new_window`.

### 4.4. Tracker & Ad Protection
- **Rust Network Blocker (`blocker.rs`):**
  - High-performance domain matcher with 60+ EasyList-lite domains.
  - Path pattern matcher: blocks `/pagead/`, `/ads.js`, `/google-analytics.com/`, `/analytics.js`, `/gtm.js`, `facebook.com/tr`, `/fbevents.js`, `clarity.ms/tag`, `stats.g.doubleclick.net`, `/pixel.gif`, `/beacon.js`, `/telemetry`, `scorecardresearch.com`, `taboola.com`, `outbrain.com`, `criteo.com`, `adnxs.com`, `googlesyndication.com`.
  - Intercepted in `on_web_resource_request`, returning `HTTP 403 Forbidden`.
- **In-Page DOM Blocker (`PAGE_INJECTION_SCRIPT`):**
  - Injects cosmetic CSS hiding matching ad containers, sponsored widgets, and ad iframes.
  - Runs `scanAndBlockAds()` on `DOMContentLoaded`, `load`, and via `MutationObserver` on DOM updates.
  - When hidden elements are detected, sends `tracker_blocked_dom?count=N` to Rust.
- **Zero-Latency Real-Time UI Sync:**
  - Whenever network or DOM blockers trigger, Rust executes:
    `tb.eval(format!("if (window.__onTrackerBlocked) {{ window.__onTrackerBlocked({}); }}", count))`
  - Immediately updates `#adblock-badge` and `#tracker-popover-count` with 0ms IPC latency.

### 4.5. Safari/Chrome-Grade Non-Blocking Download Pipeline (`downloads.rs`)
The download pipeline uses native WebView2 COM hooking with strict STA threading protections:

```text
+-----------------------------------------------------------------------------------------+
| WebView2 COM Callback (DownloadStarting / BytesReceivedChanged / StateChanged)          |
|   1. Read total bytes & proposed filename                                               |
|   2. If Ask-Location enabled: prompt file dialog (HWND::default(), never blocks owner)  |
|      - Cancel clicked -> args.SetCancel(true); args.SetHandled(true); return Ok(())     |
|   3. SetResultFilePath & SetHandled(true) -> WebView2 downloads in background network proc|
|   4. Update DownloadManager (Mutex locks dropped immediately)                            |
|   5. Throttle progress to ~200ms intervals (5Hz) to prevent IPC flooding                |
|   6. Offload disk I/O -> tauri::async_runtime::spawn_blocking (persist_async)           |
|   7. Dispatch UI update -> tauri::async_runtime::spawn (tb.eval / app.emit)             |
|   8. COM callback exits in < 1 ms! Zero UI thread / STA message pump freeze.            |
+-----------------------------------------------------------------------------------------+
```

#### Key Download Safeguards:
1. **Never block the WebView2 STA COM stack:** Any synchronous `tb.eval(...)`, synchronous file dialog `.join()`, or disk `fs::write` inside a COM callback freezes the Win32 message pump, leading to the dreaded `Shadhin-Browser (Not Responding)` hang.
2. **Asynchronous UI Dispatching:** All `tb.eval` and `app.emit` calls are spawned via `tauri::async_runtime::spawn(async move { ... })`.
3. **Asynchronous Disk Persistence:** `persist_async` runs in `tauri::async_runtime::spawn_blocking` with atomic temporary file writes (`.tmp` + rename).
4. **Clean Cancellation:** If the user clicks "Cancel" in the Save File dialog, `args.SetCancel(true)` and `args.SetHandled(true)` cancel the operation cleanly without falling back to the default folder.
5. **Platform Separation:** Generic Wry `.on_download(...)` in `commands.rs` is gated with `#[cfg(not(windows))]` to prevent conflicting with the Windows COM hook.
6. **Unique Filenames:** `resolve_unique_filepath` auto-increments collisions (`file (1).zip`, `file (2).zip`).

### 4.6. Password Manager (Windows DPAPI Vault)
- **Encryption:** Uses native Windows DPAPI (`CryptProtectData` and `CryptUnprotectData` from `windows::Win32::Security::Cryptography`).
- **Storage:** Saved in `%LOCALAPPDATA%\Shadhin-Browser\passwords.vault`.
- **Form Interception:** `PAGE_INJECTION_SCRIPT` detects password input submissions and calls `on_login_submitted`.
- **Save Banner:** Prompts user via floating banner with "Save", "Not Now", or "Never".
- **Autofill:** Injected script queries `get_credentials_for_origin` and automatically populates username and password fields on `DOMContentLoaded`.

---

## 5. Tauri IPC Command & Event Reference

### 5.1. IPC Commands (`invoke`)

| Command Name | Arguments | Returns | Description |
|---|---|---|---|
| `create_tab` | `url: Option<String>` | `Result<TabInfo, String>` | Spawns a new isolated tab webview |
| `switch_tab` | `tab_id: String` | `Result<(), String>` | Activates and displays target tab |
| `close_tab` | `tab_id: String` | `Result<(), String>` | Closes target tab webview |
| `close_active_tab`| None | `Result<(), String>` | Closes current visible tab |
| `navigate_to` | `url: String` | `Result<(), String>` | Navigates active tab to URL |
| `go_back` | None | `Result<(), String>` | Navigates active tab back in history |
| `go_forward` | None | `Result<(), String>` | Navigates active tab forward in history |
| `reload` | None | `Result<(), String>` | Reloads active tab |
| `stop` | None | `Result<(), String>` | Stops loading on active tab |
| `get_tabs` | None | `Result<Vec<TabInfo>, String>` | Returns list of open tabs |
| `get_active_tab` | None | `Result<Option<TabInfo>, String>`| Returns active tab metadata |
| `window_minimize` | None | `Result<(), String>` | Minimizes main window |
| `window_maximize` | None | `Result<(), String>` | Toggles maximize / restore |
| `window_close` | None | `Result<(), String>` | Closes application |
| `window_start_dragging` | None | `Result<(), String>` | Initiates window drag |
| `set_toolbar_expanded` | `expanded: bool, height: Option<f64>` | `Result<(), String>` | Dynamically resizes toolbar webview |
| `focus_address_bar`| None | `Result<(), String>` | Focuses toolbar webview and selects address bar |
| `get_downloads` | None | `Result<Vec<DownloadItem>, String>` | Gets persistent download list |
| `open_download_file` | `path: String` | `Result<(), String>` | Opens file with default system application |
| `open_download_folder` | `path: String` | `Result<(), String>` | Opens Windows File Explorer highlighting file |
| `clear_downloads` | None | `Result<(), String>` | Clears download history |
| `pause_download` | `id: String` | `Result<(), String>` | Pauses an active download operation via WebView2 COM |
| `resume_download` | `id: String` | `Result<(), String>` | Resumes a paused download operation |
| `cancel_download` | `id: String` | `Result<(), String>` | Cancels an ongoing download |
| `retry_download` | `id: String` | `Result<(), String>` | Re-initiates download in tab preserving cookies & auth |
| `remove_download` | `id: String` | `Result<(), String>` | Removes download entry from persistent list |
| `delete_download_file` | `id: String` | `Result<(), String>` | Deletes downloaded file from disk and removes entry |
| `pick_download_folder` | None | `Result<Option<String>, String>` | Opens native Windows folder picker dialog |
| `open_downloads_folder` | None | `Result<(), String>` | Opens configured/default downloads folder in Explorer |
| `check_download_file_exists` | `id: String` | `Result<bool, String>` | Checks if downloaded file still exists on disk |
| `get_settings` | None | `Result<AppSettings, String>` | Loads user settings |
| `save_settings` | `settings: AppSettings` | `Result<(), String>` | Saves updated settings |
| `get_passwords` | None | `Result<Vec<PasswordEntryView>, String>` | Gets DPAPI-stored passwords (masked) |
| `save_password` | `origin, username, password` | `Result<(), String>` | Encrypts & saves credentials |
| `delete_password` | `id: String` | `Result<(), String>` | Deletes saved password entry |
| `get_credentials_for_origin` | `origin: String` | `Result<Option<(String, String)>, String>` | Decrypts & retrieves credentials for autofill |
| `clear_browsing_data` | `clear_cache, clear_cookies, clear_history, clear_passwords` | `Result<(), String>` | Wipes session/local storage & selected caches |
| `new_private_window` | None | `Result<(), String>` | Launches private incognito browser window |
| `toggle_adblock` | None | `Result<bool, String>` | Toggles adblock state |
| `toggle_ad_blocker`| `enabled: bool` | `Result<bool, String>` | Sets adblock state explicitly |
| `get_blocked_count`| None | `Result<usize, String>` | Returns total blocked tracker count |
| `on_login_submitted` | `origin, username, password` | `Result<(), String>` | Emits password save prompt |
| `on_fullscreen_element_changed` | `isFullscreen: bool` | `Result<(), String>` | Hides/shows toolbar on HTML5 fullscreen |

### 5.2. Tauri Events (`listen`)

| Event Name | Payload | Direction | Purpose |
|---|---|---|---|
| `tab-created` | `TabInfo` | Backend -> UI | Tab created and registered |
| `tab-switched` | `TabInfo` | Backend -> UI | Active tab switched |
| `tab-load-status` | `{ id, isLoading, url }` | Backend -> UI | Tab navigation status updated |
| `tab-title-changed`| `{ id, title }` | Backend -> UI | Tab title updated |
| `open-url-in-new-tab` | `String (url)` | Backend -> UI | Request to open URL in a new tab |
| `download-started` | `DownloadItem` | Backend -> UI | New download initialized |
| `download-progress`| `DownloadProgressPayload` | Backend -> UI | Throttled download progress update |
| `download-state-changed` | `DownloadItem` | Backend -> UI | Download state transitioned |
| `download-finished`| `DownloadItem` | Backend -> UI | Download reached terminal state |
| `tracker-blocked` | `{ count, url }` | Backend -> UI | Ad or tracker blocked |

---

## 6. Development, Build & Release Workflows

### 6.1. Running in Development Mode
```powershell
# From project root:
cargo run --manifest-path src-tauri/Cargo.toml
```

### 6.2. Fast Offline Verification
Before committing code, always run:
```powershell
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- downloads
```

### 6.3. Building Installers
- **Build NSIS Installer (`.exe`):**
  ```powershell
  cargo tauri build
  # Output: src-tauri/target/release/bundle/nsis/Shadhin-Browser_1.1.0_x64-setup.exe
  ```
- **Build WiX MSI Installer (`.msi`):**
  ```powershell
  cargo tauri build --bundles msi
  # Output: src-tauri/target/release/bundle/msi/Shadhin-Browser_1.1.0_x64_en-US.msi
  ```

---

## 7. Inviolable Rules for AI Agents Modifying This Project

1. **NEVER BLOCK THE STA COM THREAD IN DOWNLOAD HANDLERS:**
   - Never call `tb.eval(...)` synchronously inside `DownloadStarting`, `BytesReceivedChanged`, or `StateChanged` COM callbacks. Always wrap with `tauri::async_runtime::spawn(...)`.
   - Never perform synchronous disk I/O (`fs::write`, `fs::create_dir_all`) inside download callbacks. Use `persist_async` / `tauri::async_runtime::spawn_blocking(...)`.
   - Never attach modal file dialogs to `GetForegroundWindow()` or call `.join()` on modal threads while blocking COM callbacks.
2. **DO NOT BREAK MULTI-WEBVIEW BOUNDS WITH HARD-CODED HEIGHTS:**
   - Any new popover or modal in `ui/index.html` MUST use dynamic measurement (`measureAndExpand(element)` for floating popovers, or `expandToolbar(null)` for full-window modals).
   - Ensure scroll containers have `overflow-y: auto` with thin scrollbars, and always call `collapseToolbar()` / `checkAndCollapseToolbar()` upon closing.
3. **PRESERVE MULTI-WINDOW ISOLATION:**
   - When spawning new windows, always use `WebviewWindowBuilder::new(...)` with unique profile subdirectories (`profiles/window_<uuid>` or `profiles/incognito_<uuid>`) and `decorations(true)`. Never point two windows to the same `profiles/default` directory simultaneously.
4. **PRESERVE DUAL-LAYER SHORTCUT INTERCEPTION:**
   - Register page shortcuts in `PAGE_INJECTION_SCRIPT` using `sendAction('<action>')`, handle them in `commands.rs::handle_injected_action()`, and handle toolbar fallback in `ui/main.js`.
5. **KEEP FRONTEND PURE VANILLA:**
   - Do NOT introduce npm build steps, JSX, TypeScript, webpack, vite, or frontend frameworks to `ui/`. Keep `ui/main.js`, `ui/index.html`, and `ui/styles.css` directly runnable by WebView2 without build tools.
6. **MAINTAIN WINDOWS NATIVE SECURITY:**
   - Keep DPAPI (`CryptProtectData`/`CryptUnprotectData`) for password vault storage and `windows_core` COM safety primitives.

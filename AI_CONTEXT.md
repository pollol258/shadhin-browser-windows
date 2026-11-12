# Shadhin-Browser (স্বাধীন ব্রাউজার) — AI Architecture & Technical Context

> **Target Audience:** Any AI Assistant, LLM Agent, or Senior Systems Engineer inspecting, maintaining, or extending this codebase.  
> **Repository Location:** `D:\MyProject\pollol258`  
> **Tech Stack:** Tauri 2.12.5 (Rust) + Microsoft Edge WebView2 (Windows 10/11) + Vanilla Modern ES6 JavaScript / HTML5 / CSS3 (Zero node runtime dependencies for frontend).

---

## 1. Executive Summary & Purpose

**Shadhin-Browser** is a fast, memory-efficient, privacy-focused desktop web browser for Windows. It provides an Apple Safari macOS-inspired aesthetic combined with a multi-webview child architecture powered by Microsoft Edge WebView2 and Rust.

### Key Architectural Pillars
- **Zero Heavy Frontend Frameworks:** The UI is completely vanilla (HTML5, CSS3, ES6 JS). No React, Vue, or Webpack bundle overhead.
- **Hardware/OS-Level DPAPI Encryption:** Saved website passwords are encrypted directly through Windows Data Protection API (`CryptProtectData` / `CryptUnprotectData`), securing them against cross-account or cross-machine extraction.
- **Dual-Layer Tracker & Ad Protection:** Combines a Rust-level network interceptor (`on_web_resource_request` with EasyList-lite and telemetry path blocking) and an in-page DOM cosmetic blocker with `MutationObserver`.
- **Dynamic Multi-Webview Bounds Management:** Overcomes child webview clipping by dynamically resizing the top chrome webview when popovers or modals open, while maintaining transparency over underlying web content.
- **Dual-Layer Keyboard Shortcut Interception:** Injects a global shortcut capture listener into content pages (`http://shadhin.action/<action>`) bypassing cross-origin iframe and CSP restrictions.

---

## 2. Architecture & Multi-Webview Design

Shadhin-Browser uses Tauri 2's **Multi-Webview Child Architecture**:

```
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

### 2.1. The Critical Multi-Webview Bounds Constraint & Fix
- **Constraint:** In Tauri 2 / WebView2, a child webview physically clips all HTML content outside its bounding rectangle. The chrome toolbar webview is physically positioned at `(0.0, 0.0)` with a default height of `82.0px` (`TOOLBAR_HEIGHT`). Any popover or modal rendered at `top: 90px` or full screen is clipped and 100% invisible.
- **The Solution (Dynamic Toolbar Expansion):**
  1. The toolbar webview is instantiated with `.transparent(true)` in [lib.rs](file:///d:/MyProject/pollol258/src-tauri/src/lib.rs), and `html, body` has `background: transparent;`.
  2. When any overlay opens, the frontend calls `set_toolbar_expanded(expanded: true, height: Option<f64>)`.
     - Downloads Popover: Expands to `480px`.
     - Safari Menu: Expands to `360px`.
     - Tracker Protection Popover: Expands to `340px`.
     - Settings / About Modal: Expands to full window height (`null`).
     - Custom Confirm Dialog: Expands to `260px`.
     - Password Save Banner: Expands to `220px`.
  3. Because everything outside the 82px chrome header is completely transparent, the popover card floats seamlessly over the underlying web page.
  4. When closed, outside-clicked, or when `Esc` is pressed, `collapseToolbar()` immediately resets the toolbar height back to `TOOLBAR_HEIGHT = 82.0px`.

---

## 3. Directory & File Structure

```text
D:\MyProject\pollol258/
├── src-tauri/                          # Rust Backend Core
│   ├── src/
│   │   ├── main.rs                    # Application entry point (calls shadhin_browser_lib::run())
│   │   ├── lib.rs                     # Window setup, toolbar builder, resize listeners, command registration
│   │   ├── commands.rs                # All Tauri IPC commands & shortcut dispatchers
│   │   ├── blocker.rs                 # EasyList-lite network blocker & atomic counter
│   │   ├── downloads.rs               # Download manager & persistent JSON history
│   │   ├── passwords.rs               # Windows DPAPI hardware-bound password vault
│   │   ├── settings.rs                # User settings (search engine, homepage, adblock flags)
│   │   ├── fullscreen.rs              # HTML5 fullscreen state tracking
│   │   └── browser/
│   │       ├── mod.rs                 # PAGE_INJECTION_SCRIPT & toolbar height constants
│   │       ├── navigation.rs          # URL normalization & search engine query formatting
│   │       └── tabs.rs                # TabManager data structures & active tab state
│   ├── capabilities/
│   │   └── default.json               # Tauri 2 security permissions & capabilities
│   ├── tauri.conf.json                # Tauri 2 configuration (productName, icons, bundle, withGlobalTauri)
│   └── Cargo.toml                     # Rust dependencies (tauri 2.12.5, windows 0.58, serde, dirs, uuid, etc.)
├── ui/                                # Frontend UI (Zero Node build dependencies)
│   ├── index.html                     # Safari layout (traffic lights, address bar, popovers, modals)
│   ├── styles.css                     # Apple Safari macOS theme (light/dark mode, acrylic blur, drag regions)
│   └── main.js                        # UI controller, IPC caller, keyboard listener, tab strip renderer
├── .gitignore                         # Git ignore file (target/, tauri-2.12.5/ ignored)
├── README.md                          # Human-facing project overview
└── AI_CONTEXT.md                      # This file (AI and developer architecture manual)
```

---

## 4. Key Subsystems & Implementation Details

### 4.1. Tab Management (`tabs.rs`, `commands.rs`, `main.js`)
- **Backend:** `TabManager` holds a `Vec<TabInfo>` and `active_tab_id: Option<String>`.
- **Lifecycle:**
  - `create_tab(url)`: Hides the currently active tab webview, instantiates a new child webview with `PAGE_INJECTION_SCRIPT`, sets its size to `(width, logical_height - 82.0)`, attaches it, and emits `"tab-created"` and `"tab-switched"`.
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

### 4.3. Dual-Layer Keyboard Shortcuts
Shortcuts must work even when keyboard focus is inside a third-party webpage or foreign iframe.
1. **In-Page Content Script (`PAGE_INJECTION_SCRIPT`):**
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
   - `Ctrl + L` / `Alt + D` / `F6`: Focus address bar (invokes `focus_address_bar` -> `tb.set_focus()` + select all)
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

### 4.4. Address Bar & Navigation
- **`Ctrl + Enter` (or `Cmd + Enter`):** Automatically completes `<text>` into `https://www.<text>.com`.
- **Smart URL Normalization:**
  - If text contains `.` without spaces or starts with `http://`, `https://`, `localhost`, it navigates directly.
  - Otherwise, it queries the configured search engine (`DuckDuckGo`, `Google`, `Bing`, or `Brave`).

### 4.5. Page Zoom In / Out
- Enabled `.zoom_hotkeys_enabled(true)` on `WebviewBuilder`.
- In `PAGE_INJECTION_SCRIPT`:
  - `Ctrl + Mouse Wheel`: Increments/decrements `document.documentElement.style.zoom` in steps of `0.1` between `0.3x` and `3.0x`.
  - `Ctrl + +`: Zoom in.
  - `Ctrl + -`: Zoom out.
  - `Ctrl + 0`: Reset zoom to 1.0x.

### 4.6. Tracker & Ad Protection
- **Rust Network Blocker (`blocker.rs`):**
  - High-performance zero-allocation domain matcher with 60+ EasyList-lite domains.
  - Path pattern matcher: blocks `/pagead/`, `/ads.js`, `/google-analytics.com/`, `/analytics.js`, `/gtm.js`, `facebook.com/tr`, `/fbevents.js`, `clarity.ms/tag`, `stats.g.doubleclick.net`, `/pixel.gif`, `/beacon.js`, `/telemetry`, `scorecardresearch.com`, `taboola.com`, `outbrain.com`, `criteo.com`, `adnxs.com`, `googlesyndication.com`.
  - Intercepted in `on_web_resource_request`, returning `HTTP 403 Forbidden`.
- **In-Page DOM Blocker (`PAGE_INJECTION_SCRIPT`):**
  - Injects cosmetic CSS hiding matching ad containers, sponsored widgets, and ad iframes.
  - Runs `scanAndBlockAds()` on `DOMContentLoaded`, `load`, and via `MutationObserver` on DOM updates.
  - When hidden elements are detected, sends `tracker_blocked_dom?count=N` to Rust.
- **Zero-Latency Real-Time UI Sync:**
  - Whenever network or DOM blockers trigger, Rust executes:
    `tb.eval("if (window.__onTrackerBlocked) { window.__onTrackerBlocked(count); }")`
  - Immediately updates `#adblock-badge` and `#tracker-popover-count` with 0ms IPC latency.

### 4.7. Download Manager (`downloads.rs`, `commands.rs`, `main.js`)
- **WebView2 `on_download` Interception:**
  - `Requested { url, destination }`:
    - **Safe Filename Resolution:** WebView2 often passes an empty `destination`. Shadhin-Browser extracts the filename from `destination.file_name()`, or parses the last segment of `url.path_segments()`, sanitizes Windows-forbidden characters (`\ / : * ? " < > |`), and sets `*destination = target_dir.join(&safe_fname)`.
    - Emits `"download-started"` and calls `tb.eval("window.__onDownloadStarted(...)")`.
  - `Finished { url, success }`:
    - Locates the active download item (with fallback for redirected URLs), updates status to `"completed"` or `"failed"`, and calls `tb.eval("window.__onDownloadFinished(...)")`.
    - Persists history in `%LOCALAPPDATA%\Shadhin-Browser\downloads.json`.
- **Auto-Open UX:** Upon download completion, `ui/main.js` automatically pops open `#downloads-panel`, displaying the filename, status, and **"Open File"** / **"Show in Folder"** buttons.
- **`target="_blank"` Rewrite:** In `PAGE_INJECTION_SCRIPT`, any link click with `target="_blank"` pointing to downloadable extensions (`.zip`, `.exe`, `.pdf`, `.apk`, `.iso`, etc.) is rewritten to `target="_self"` so downloads don't get lost in unhandled popup tabs.

### 4.8. Password Manager (Windows DPAPI Vault)
- **Encryption:** Uses native Windows DPAPI (`CryptProtectData` and `CryptUnprotectData` from `windows::Win32::Security::Cryptography`).
- **Storage:** Saved in `%LOCALAPPDATA%\Shadhin-Browser\passwords.vault`.
- **Form Interception:** `PAGE_INJECTION_SCRIPT` detects password input submissions and calls `on_login_submitted`.
- **Save Banner:** Prompts user via floating banner with "Save", "Not Now", or "Never".
- **Autofill:** Injected script queries `get_credentials_for_origin` and automatically populates username and password fields on `DOMContentLoaded`.

---

## 5. Tauri IPC Command Reference

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

---

## 6. Development & Build Workflows

### 6.1. Running in Development Mode
```powershell
# From project root:
cargo run --manifest-path src-tauri/Cargo.toml
```
*Note: Debug executable output is placed at `src-tauri/target/debug/shadhin-browser.exe`.*

### 6.2. Fast Offline Verification
```powershell
cargo check --manifest-path src-tauri/Cargo.toml --offline
node -c ui/main.js
```

### 6.3. Building the Windows Installer (`.msi`)
To generate a production `.msi` (WiX installer):
```powershell
# From project root:
npx @tauri-apps/cli build --bundles msi
```
The output installer is located at:
```text
D:\MyProject\pollol258\src-tauri\target\release\bundle\msi\Shadhin-Browser_1.0.0_x64_en-US.msi
```

---

## 7. Guidelines for AI Agents Modifying This Project

1. **Do NOT break Multi-Webview Bounds:** Any new popover or modal added to `ui/index.html` MUST call `expandToolbar(height)` upon opening and `checkAndCollapseToolbar()` upon closing.
2. **Preserve Dual-Layer Interception:** When adding new keyboard shortcuts or page-level actions, register them in `PAGE_INJECTION_SCRIPT` using `sendAction('<action>')`, handle them in `commands.rs::handle_injected_action()`, and handle fallback in `ui/main.js`.
3. **Use Direct `tb.eval` for Critical UI Notifications:** Do not rely solely on `app.emit()` across webviews; call `tb.eval("window.__onEvent(...)")` on `"toolbar"` for guaranteed 0ms response time.
4. **Maintain Windows Native APIs:** Keep DPAPI (`CryptProtectData`) for credentials and `explorer.exe /select,<path>` for downloads.
5. **Keep Frontend Vanilla:** Do not introduce npm build steps, JSX, TypeScript, or heavy front-end frameworks to `ui/`.

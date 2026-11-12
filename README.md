# Shadhin-Browser (স্বাধীন ব্রাউজার)
### Production-Quality Windows Desktop Browser Built with Tauri 2 + Rust + WebView2

---

## 🌟 Overview

**Shadhin-Browser** is a fast, memory-efficient, privacy-first desktop web browser for Windows 10/11. It combines a native **Apple Safari-inspired frameless header** with an isolated **multi-webview child architecture** powered by Microsoft WebView2 and Rust.

All sensitive data (saved website logins) is encrypted with hardware/OS-bound **Windows DPAPI** (`CryptProtectData`), and built-in network filtering blocks trackers and ads without third-party extensions.

---

## 🏗️ Architecture

Shadhin-Browser uses Tauri 2's native **Multi-Webview Child Architecture**:

```mermaid
graph TD
    subgraph Host Window ["Main Window (Frameless, decorations: false, Windows 10/11)"]
        TB["Toolbar Webview (Safari UI, y: 0, height: 82px)<br/>- Safari Tab Strip (Active Tabs, Add +)<br/>- Back / Forward / Bookmark<br/>- Center Address Group + Security Lock + Progress<br/>- Downloads, Menu, Traffic Lights (Min/Max/Close)"]
        
        subgraph Content Area ["Content Child Webviews (y: 82px, height: window_height - 82px)"]
            T1["Tab 1 Webview2 (Active, Visible)<br/>Isolated Session / WebView2 Engine"]
            T2["Tab 2 Webview2 (Background, Hidden)<br/>Suspended in Memory"]
            TN["Tab N Webview2 (Background, Hidden)"]
        end
    end

    subgraph Rust Core Backend ["Rust Backend (Tauri 2 Core)"]
        TM["Tab Manager<br/>tabs.rs / navigation.rs"]
        DM["Download Manager<br/>downloads.rs / Shell API"]
        PM["DPAPI Vault<br/>passwords.rs (CryptProtectData)"]
        AB["Tracker & Ad Blocker<br/>blocker.rs (Hosts + EasyList)"]
        FS["HTML5 Fullscreen Manager<br/>fullscreen.rs"]
        SM["Settings Manager<br/>settings.rs"]
    end

    TB -->|Tauri IPC Commands| Rust Core Backend
    Rust Core Backend -->|Tauri Events (tab-updated, download-started, etc.)| TB
    T1 -.->|Injected JS / IPC (Logins, Fullscreen, Shortcuts)| Rust Core Backend
```

### Component Details
1. **One Main Window (`main`)**: A frameless native window (`decorations: false`, `shadow: true`, `resizable: true`).
2. **Top Chrome (`toolbar`)**: An internal HTML/CSS/JS webview loaded from `ui/index.html` taking the top 82px. It renders the exact Safari macOS-style controls with dynamic tabs. Elements use `data-tauri-drag-region` for smooth window movement.
3. **Tab Manager (`tabs.rs`)**: Each tab is an isolated child `WebView2` attached via `window.add_child`. Inactive tabs are hidden (`webview.hide()`), and the active tab is shown and sized to fill the content area.
4. **Hardware-Backed Password Vault (`passwords.rs`)**: Encrypts and decrypts credentials using **Windows DPAPI** (`CryptProtectData` and `CryptUnprotectData`). Encrypted blobs cannot be decrypted on other user accounts or machines.
5. **Built-in Ad & Tracker Blocker (`blocker.rs`)**: Intercepts web resource requests using `on_web_resource_request` before they hit the network, terminating connections to known advertising and telemetry domains with `403 Forbidden`.
6. **Download Manager (`downloads.rs`)**: Intercepts native WebView2 downloads, manages destinations, emits live events to the UI, and persists records in `%APPDATA%\Shadhin-Browser\downloads.json`.

---

## 📂 Project Structure

```text
pollol258/
├── ui/                                # Clean, vanilla frontend (Zero Node dependencies)
│   ├── index.html                     # Exact Safari chrome layout (tabs, address, traffic lights, modals)
│   ├── styles.css                     # Apple Safari macOS styling + Dark mode support
│   └── main.js                        # UI logic, event listeners, Tauri IPC wrappers
│
└── src-tauri/                         # Rust Backend (Tauri 2.x)
    ├── Cargo.toml                     # Rust dependencies (tauri 2, windows 0.58, tokio, etc.)
    ├── tauri.conf.json                # Tauri v2 configuration (frameless, security, bundler)
    ├── build.rs                       # Build script
    ├── capabilities/
    │   └── default.json               # Window & webview permissions configuration
    ├── icons/                         # Windows application icons (ico, png)
    └── src/
        ├── main.rs                    # Application entry point
        ├── lib.rs                     # Runtime setup, window creation, child webview layout
        ├── commands.rs                # Tauri IPC commands & shared app state
        ├── downloads.rs               # Download manager (events, shell open, persistence)
        ├── passwords.rs               # Windows DPAPI encryption & credentials vault
        ├── blocker.rs                 # Tracker & Ad blocker filter (Hosts & EasyList-lite)
        ├── fullscreen.rs              # HTML5 fullscreen listener (YouTube, Facebook, Netflix)
        ├── settings.rs                # User preferences persistence
        └── browser/
            ├── mod.rs                 # Webview injection scripts (forms, shortcuts, fullscreen)
            ├── tabs.rs                # Tab manager, active state, bounds calculator
            └── navigation.rs          # URL parsing & DuckDuckGo search fallback
```

---

## 🚀 Step-by-Step Run Instructions

### 1. Prerequisites
- **Windows 10 or 11 (64-bit)**
- **Rust Toolchain (stable)**: `rustc`, `cargo`
- **Microsoft Edge WebView2 Runtime**: Pre-installed on all Windows 10/11 systems. (If missing, install the [WebView2 Evergreen Bootstrapper](https://go.microsoft.com/fwlink/p/?LinkId=2124703)).

### 2. Run in Development Mode
You can run the browser directly with Cargo or using the Tauri CLI:

#### Using Cargo:
```powershell
cd D:\MyProject\pollol258\src-tauri
cargo run
```

#### Using Tauri CLI:
```powershell
# From project root:
cargo tauri dev
```

### 3. Build Production Installer (MSI / NSIS)
```powershell
cd D:\MyProject\pollol258\src-tauri
cargo tauri build
```
The output installers will be generated under:
`src-tauri/target/release/bundle/nsis/` (Single `.exe` installer)  
`src-tauri/target/release/bundle/msi/` (Windows `.msi` package)

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + T` | Open new tab |
| `Ctrl + W` | Close active tab |
| `Ctrl + L` | Focus address bar |
| `Ctrl + R` / `F5` | Reload page |
| `Alt + Left` | Navigate back |
| `Alt + Right` | Navigate forward |
| `Ctrl + Shift + N` | Open new private window (in-memory incognito profile) |
| `Ctrl + ,` | Open Settings & Preferences |
| `Esc` | Stop loading / dismiss popovers |

---

## 🛡️ Security & Privacy Architecture

- **Context Isolation**: Web pages run in isolated child WebView2 instances with restricted IPC privileges. Untrusted page code cannot invoke arbitrary native commands.
- **Windows DPAPI Vault**: Passwords saved on websites are encrypted with machine/user-bound Windows DPAPI (`CryptProtectData`). They cannot be read outside the active Windows user session.
- **Zero Telemetry**: No phone-home calls, no crash analytics, no external tracking.
- **Private Browsing**: Private windows utilize ephemeral data directories (`incognito_<uuid>`) that do not write cookies, history, or cache to persistent storage.

# Shadhin-Browser (Windows Edition)
### Production-Quality Windows Desktop Browser Built with Rust, Tauri v2 & WebView2

---

## 📥 Download & Install (Windows)

🚀 **[Download Latest Windows Release (.exe)](https://github.com/pollol258/shadhin-browser-windows/releases/latest)**

> [!NOTE]
> **Windows SmartScreen Notice:** Since this installer is an open-source community release without an enterprise EV code signing certificate, Windows SmartScreen may display a *"Windows protected your PC"* prompt on first launch. Simply click **"More info"** → **"Run anyway"** to install and launch the browser.

---

## 🌟 Overview

**Shadhin Browser (Windows Edition)** is a fast, memory-efficient, and privacy-first desktop web browser for Windows 10 and 11. It pairs an Apple Safari-inspired frameless header with an isolated multi-webview child architecture powered by Microsoft WebView2 and Rust.

---

## 🚀 Key Features

- **Brave Adblock & EasyList Integration**: Native web-resource interception that filters and terminates ad and tracker requests before they hit the network.
- **Privacy Shield & Anti-Fingerprinting**: Comprehensive fingerprinting mitigation, WebRTC leak protection, and zero telemetry.
- **Live Tracking Counter**: Real-time counter in the toolbar displaying blocked trackers and ads per session and per page.
- **Hardware-Backed DPAPI Password Vault**: Saved credentials encrypted using Windows DPAPI (`CryptProtectData`), bound to your Windows user session and TPM/hardware.
- **Full Bookmarks Manager**: Native JSON-backed bookmark storage (`bookmarks.rs`) with quick toolbar access and folder organization.
- **Multi-Window Support & Tab Tear-Off**: Detach tabs into isolated native windows or open links directly in new windows and private browsing windows with isolated profiles.
- **Integrated Download Manager**: Real-time download progress, custom save dialogs, and persistent download history.
- **HTML5 Fullscreen Support**: Smooth transition handling for YouTube, Netflix, Facebook video players, and web apps.

---

## 🏗️ Architecture

Shadhin Browser uses Tauri 2's native **Multi-Webview Child Architecture**:

```mermaid
graph TD
    subgraph Host Window ["Main Window (Frameless, decorations: false, Windows 10/11)"]
        TB["Toolbar Webview (Safari UI, y: 0, height: 82px)<br/>- Tab Strip & Tab Management<br/>- Navigation & Address Group<br/>- Shield Counter, Bookmarks, Downloads"]
        
        subgraph Content Area ["Content Child Webviews (y: 82px, height: window_height - 82px)"]
            T1["Tab 1 Webview2 (Active, Visible)<br/>Isolated Session / WebView2 Engine"]
            T2["Tab 2 Webview2 (Background, Hidden)<br/>Suspended in Memory"]
            TN["Tab N Webview2 (Background, Hidden)"]
        end
    end

    subgraph Rust Core Backend ["Rust Backend (Tauri 2 Core)"]
        TM["Tab Manager (tabs.rs / navigation.rs)"]
        DM["Download Manager (downloads.rs)"]
        BM["Bookmarks Manager (bookmarks.rs)"]
        PM["DPAPI Vault (passwords.rs)"]
        AB["Privacy Shield & Adblocker (blocker.rs)"]
        FS["HTML5 Fullscreen Manager (fullscreen.rs)"]
        SM["Settings Manager (settings.rs)"]
    end

    TB -->|Tauri IPC Commands| Rust Core Backend
    Rust Core Backend -->|Tauri Events| TB
    T1 -.->|Injected JS / IPC Actions| Rust Core Backend
```

---

## 📂 Project Structure

```text
pollol258/
├── ui/                                # Clean, vanilla frontend (Zero Node dependencies)
│   ├── index.html                     # Safari-inspired chrome layout
│   ├── styles.css                     # Apple Safari styling + Dark mode support
│   ├── main.js                        # UI logic, event listeners, Tauri IPC wrappers
│   └── newtab.html                    # Native speed-dial / new tab page
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
        ├── bookmarks.rs               # Bookmarks manager
        ├── downloads.rs               # Download manager (events, shell open, persistence)
        ├── passwords.rs               # Windows DPAPI encryption & credentials vault
        ├── blocker.rs                 # Tracker & Ad blocker filter (Brave/EasyList)
        ├── fullscreen.rs              # HTML5 fullscreen listener
        ├── settings.rs                # User preferences persistence
        └── browser/
            ├── mod.rs                 # Webview injection scripts (forms, shortcuts, fullscreen)
            ├── tabs.rs                # Tab manager, active state, bounds calculator
            └── navigation.rs          # URL parsing & DuckDuckGo search fallback
```

---

## 🛠️ Build & Development Instructions

### Prerequisites
- **Windows 10 or 11 (64-bit)**
- **Rust Toolchain**: `rustup`, `rustc`, `cargo` (stable)
- **Microsoft Edge WebView2 Runtime**: Pre-installed on Windows 10/11 (or install via [WebView2 Evergreen Bootstrapper](https://go.microsoft.com/fwlink/p/?LinkId=2124703)).
- **Tauri CLI**: `cargo install tauri-cli --version "^2.0.0"`

### 1. Run in Development Mode
From the project root:
```powershell
cargo tauri dev
```

Or from the `src-tauri` directory:
```powershell
cd src-tauri
cargo run
```

### 2. Build Production Installer (MSI / NSIS)
```powershell
cargo tauri build
```
The output installers will be generated under:
- `src-tauri/target/release/bundle/nsis/` (Single `.exe` installer)
- `src-tauri/target/release/bundle/msi/` (Windows `.msi` package)

---

## ⌨️ Default Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + T` | Open new tab |
| `Ctrl + W` | Close active tab |
| `Ctrl + L` | Focus address bar |
| `Ctrl + R` / `F5` | Reload page |
| `Alt + Left` | Navigate back |
| `Alt + Right` | Navigate forward |
| `Ctrl + Shift + N` | Open new private window |
| `Ctrl + B` | Toggle Bookmarks bar / view |
| `Ctrl + J` | Open Downloads manager |
| `Ctrl + ,` | Open Settings & Preferences |
| `Esc` | Stop loading / dismiss popovers |

---

## 📄 License & Author

- **Author**: Md. Ziaur Rahman (<zia.r@aol.com>)
- **License**: GNU General Public License v3.0 or later ([GPL-3.0-or-later](LICENSE))

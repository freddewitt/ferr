# FERR-APP — Build Instructions for Claude Code

You are building **ferr-app**, a desktop GUI for the `ferr` CLI tool, using **Tauri v2**.
ferr is a secure file copy tool with cryptographic verification, PAR2 redundancy,
and DIT (Digital Imaging Technician) automation features.

The GUI is built with Tauri: a Rust backend (thin shell) + HTML/CSS/JS frontend.
It produces a single native `.app` (macOS), `.exe` (Windows), or AppImage (Linux).

## CRITICAL RULES

1. The GUI **never** does file I/O, hashing, or data processing directly.
2. Every action calls the `ferr` CLI binary via Tauri's sidecar API in the Rust backend.
3. The ferr binary is bundled as a Tauri sidecar (`ferr-cli` external binary).
4. Frontend: vanilla HTML/CSS/JS (no React, no framework). Keep it simple.
5. Backend: Rust with Tauri v2 — minimal code, just CLI bridge functions.
6. All UI text in English by default, with support for French.
7. Design: clean, flat, minimal. Lots of whitespace. No gradients, no shadows.
8. Target: macOS (primary), Windows + Linux (secondary, same codebase).

---

## APP STRUCTURE

```
ferr-app/
├── Cargo.toml                          # Tauri Rust backend
├── tauri.conf.json                     # Tauri config: window, sidecar, bundle
│
├── src/                                # Rust backend
│   ├── main.rs                         # Tauri entry point
│   ├── bridge.rs                       # Core: spawn ferr CLI, parse stdout
│   ├── commands.rs                     # #[tauri::command] functions exposed to JS
│   ├── progress.rs                     # Parse machine-readable progress lines
│   └── volume.rs                       # Detect mounted volumes + disk usage
│
├── ui/                                 # Frontend (HTML/CSS/JS)
│   ├── index.html                      # Single HTML entry point
│   ├── css/
│   │   ├── main.css                    # Global styles, variables, layout
│   │   ├── sidebar.css                 # Sidebar navigation
│   │   ├── components.css              # Drop zones, pills, cards, toggles
│   │   └── tabs.css                    # Tab-specific styles
│   │
│   ├── js/
│   │   ├── app.js                      # Main app: routing, state, init
│   │   ├── bridge.js                   # JS wrapper around Tauri invoke() calls
│   │   ├── tabs/
│   │   │   ├── copy.js                 # Copy tab logic
│   │   │   ├── watch.js                # Watch tab logic
│   │   │   ├── health.js               # Health tab (scan, verify, repair, cert)
│   │   │   ├── history.js              # History tab logic
│   │   │   └── settings.js             # Settings tab logic
│   │   │
│   │   ├── components/
│   │   │   ├── dropzone.js             # FolderDropZone / FileDropZone
│   │   │   ├── progress.js             # Progress overlay
│   │   │   ├── pills.js                # Status pills in topbar
│   │   │   └── healthcard.js           # Selectable health action card
│   │   │
│   │   └── utils/
│   │       ├── settings.js             # Read/write settings (Tauri store plugin)
│   │       ├── i18n.js                 # Localization helper
│   │       └── format.js               # File size, speed, time formatting
│   │
│   └── locales/
│       ├── en.json                     # English strings
│       └── fr.json                     # French strings
│
└── binaries/                           # Sidecar: ferr CLI binary
    └── ferr-cli-aarch64-apple-darwin   # (Tauri sidecar naming convention)
```

---

## TAURI CONFIGURATION

### tauri.conf.json (key sections)

```json
{
  "build": {
    "frontendDist": "ui"
  },
  "app": {
    "windows": [
      {
        "title": "ferr",
        "width": 640,
        "height": 540,
        "minWidth": 580,
        "minHeight": 480,
        "resizable": true,
        "decorations": true
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "identifier": "com.ferr.app",
    "icon": ["icons/icon.png"],
    "externalBin": ["binaries/ferr-cli"]
  }
}
```

### Sidecar setup

Tauri's `externalBin` bundles the ferr binary as a sidecar. The binary must follow
Tauri's naming convention: `ferr-cli-{target-triple}` where target-triple is e.g.
`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`.

Place the compiled ferr binaries in `binaries/`:
```
binaries/
├── ferr-cli-aarch64-apple-darwin
├── ferr-cli-x86_64-apple-darwin
├── ferr-cli-x86_64-pc-windows-msvc
└── ferr-cli-x86_64-unknown-linux-gnu
```

---

## RUST BACKEND (src/)

The Rust code is minimal — just a bridge between JS frontend and ferr CLI.

### main.rs

```rust
mod bridge;
mod commands;
mod progress;
mod volume;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::run_copy,
            commands::run_copy_preview,
            commands::run_watch_start,
            commands::run_watch_stop,
            commands::run_scan,
            commands::run_verify,
            commands::run_repair,
            commands::run_cert_create,
            commands::run_cert_verify,
            commands::run_export,
            commands::run_report,
            commands::get_history,
            commands::search_history,
            commands::get_profiles,
            commands::save_profile,
            commands::get_volumes,
            commands::pick_folder,
            commands::pick_file,
            commands::pick_save_location,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ferr-app");
}
```

### commands.rs — Pattern for streaming commands

```rust
use tauri::Manager;
use tauri_plugin_shell::ShellExt;

#[tauri::command]
async fn run_copy(
    app: tauri::AppHandle,
    source: String,
    destinations: Vec<String>,
    args: Vec<String>,
) -> Result<(), String> {
    let mut cmd_args = vec!["copy".to_string(), source, destinations[0].clone()];
    if destinations.len() > 1 { cmd_args.extend(["--dest2".into(), destinations[1].clone()]); }
    if destinations.len() > 2 { cmd_args.extend(["--dest3".into(), destinations[2].clone()]); }
    cmd_args.extend(args);
    cmd_args.extend(["--progress-format".into(), "machine".into()]);

    let sidecar = app.shell().sidecar("ferr-cli").unwrap().args(&cmd_args);
    let (mut rx, _child) = sidecar.spawn().map_err(|e| format!("Failed to start ferr: {}", e))?;

    while let Some(event) = rx.recv().await {
        match event {
            tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                app.emit("ferr-progress", String::from_utf8_lossy(&line).to_string()).ok();
            }
            tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                app.emit("ferr-error", String::from_utf8_lossy(&line).to_string()).ok();
            }
            tauri_plugin_shell::process::CommandEvent::Terminated(status) => {
                app.emit("ferr-complete", status.code.unwrap_or(-1)).ok();
            }
            _ => {}
        }
    }
    Ok(())
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let result = app.dialog().file().blocking_pick_folder();
    Ok(result.map(|p| p.to_string()))
}

// Same pattern for all other commands — see full mapping below.
```

---

## FRONTEND (ui/)

### JS bridge (js/bridge.js)

```javascript
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const Bridge = {
    pickFolder: ()           => invoke('pick_folder'),
    pickFile: (ext)          => invoke('pick_file', { extensions: ext || [] }),
    pickSaveLocation: (name) => invoke('pick_save_location', { defaultName: name }),

    runCopy: (src, dests, args)     => invoke('run_copy', { source: src, destinations: dests, args }),
    runCopyPreview: (src, dests, args) => invoke('run_copy_preview', { source: src, destinations: dests, args }),

    startWatch: (folder, dests, args) => invoke('run_watch_start', { folder, destinations: dests, args }),
    stopWatch: ()                     => invoke('run_watch_stop'),

    runScan: (folder, since)          => invoke('run_scan', { folder, sinceDate: since }),
    runVerify: (srcOrManifest, dest)  => invoke('run_verify', { sourceOrManifest: srcOrManifest, dest }),
    runRepair: (folder)               => invoke('run_repair', { folder }),
    certCreate: (folder, out)         => invoke('run_cert_create', { folder, outputPath: out }),
    certVerify: (cert, folder)        => invoke('run_cert_verify', { certPath: cert, folder }),

    exportALE: (manifest, out)        => invoke('run_export', { manifestPath: manifest, format: 'ale', outputPath: out }),
    exportCSV: (manifest, out)        => invoke('run_export', { manifestPath: manifest, format: 'csv', outputPath: out }),
    generateReport: (manifest, out)   => invoke('run_report', { manifestPath: manifest, outputPath: out }),

    getHistory: ()        => invoke('get_history').then(JSON.parse),
    searchHistory: (q)    => invoke('search_history', { query: q }).then(JSON.parse),
    getProfiles: ()       => invoke('get_profiles').then(JSON.parse),
    saveProfile: (name)   => invoke('save_profile', { name }),
    getVolumes: ()        => invoke('get_volumes'),

    onProgress: (cb) => listen('ferr-progress', e => cb(e.payload)),
    onError: (cb)    => listen('ferr-error', e => cb(e.payload)),
    onComplete: (cb) => listen('ferr-complete', e => cb(e.payload)),
};
```

### App state & routing (js/app.js)

```javascript
const App = {
    state: {
        activeTab: 'copy',
        source: null,
        destinations: [],
        copyInProgress: false,
        watchActive: false,
        watchFolder: null,
        watchDestinations: [],
        selectedHealthAction: 'scan',
    },

    init() {
        this.renderSidebar();
        this.renderTopBar();
        this.renderBottomBar();
        this.switchTab('copy');
        this.loadVolumes();
        Bridge.onProgress(line => this.handleProgress(line));
        Bridge.onComplete(code => this.handleComplete(code));
        Bridge.onError(line => this.handleError(line));
    },

    switchTab(name) {
        this.state.activeTab = name;
        // Update sidebar active state, render tab content, update topbar/bottombar
    },

    buildCopyArgs() {
        const s = Settings.getAll();
        const args = ['--hash', s.hashAlgorithm];
        if (s.par2Enabled) args.push('--par2', String(s.par2Percent));
        if (s.videoMode) {
            args.push('--camera', '--rename', s.renameTemplate);
            if (s.historyDedup) args.push('--dedup');
        }
        if (s.ejectAfterCopy) args.push('--eject');
        if (!s.preserveMetadata) args.push('--no-preserve-meta');
        if (!s.pdfReport) args.push('--no-pdf');
        return args;
    },
};

document.addEventListener('DOMContentLoaded', () => App.init());
```

---

## COMPLETE CLI ↔ GUI MAPPING

### Commands → GUI locations

| CLI command | GUI location | Trigger |
|---|---|---|
| `ferr copy <src> <dest>` | **Copy tab** | "Copy" button |
| `ferr copy ... --resume` | **Copy tab** | Auto-detected → "Resume" button |
| `ferr copy ... --dry-run` | **Copy tab** | "Preview" button |
| `ferr watch <dir> --dest <d>` | **Watch tab** | "Start watching" |
| `ferr scan <folder>` | **Health → Scan** | "Run scan" |
| `ferr scan <folder> --since <date>` | **Health → Scan → "Since date"** | Date picker + "Run scan" |
| `ferr verify <folderA> <folderB>` | **Health → Verify → "Two folders"** | "Verify" |
| `ferr verify <manifest> <folder>` | **Health → Verify → "JSON manifest"** | "Verify" |
| `ferr cert verify <cert> <folder>` | **Health → Verify → "Certificate"** | "Verify" |
| `ferr repair <folder>` | **Health → Repair** | "Repair" (confirmation) |
| `ferr cert create <folder> --output <path>` | **Health → Certificate → "Create"** | "Generate" |
| `ferr cert verify <cert> <folder>` | **Health → Certificate → "Verify"** | "Verify" |
| `ferr export <manifest> --format ale` | **History → row context menu** | "Export ALE" |
| `ferr export <manifest> --format csv` | **History → row context menu** | "Export CSV" |
| `ferr report <manifest> --output <pdf>` | **History → row context menu** | "Generate PDF" |
| `ferr history list --json` | **History tab** | Auto-loaded |
| `ferr history find <query> --json` | **History → search bar** | On input |
| `ferr profile save <n>` | **Settings → Profiles** | "Save current" button |
| `ferr profile list` | **Settings → Profiles** | Auto-loaded |

### Copy flags → Settings

| CLI flag | Control | Location |
|---|---|---|
| `--hash` | Radio: XXH64 / SHA-256 | Settings → Hash |
| `--par2 <N>` | Toggle + slider 1-30% | Settings → PAR2 |
| `--camera` | Toggle | Settings → Video mode |
| `--rename` | Text field | Settings → Video → Template |
| `--eject` | Toggle | Settings → Eject |
| `--dedup` | Toggle | Settings → Video → Dedup |
| `--no-preserve-meta` | Toggle (inverted) | Settings → Preserve metadata |
| `--no-pdf` | Toggle (inverted) | Settings → Reports → PDF |
| `--resume` | Auto-detected | Copy tab button swap |
| `--dry-run` | Button | Copy tab "Preview" |
| `--dest2/3` | Dynamic drop zones | Copy tab "+ mirror" |
| `--profile` | Dropdown | Copy tab topbar |

---

## TAB SPECIFICATIONS

### Copy tab
ONLY: two drop zones (Source + Destination), "+ Add mirror" link.
Bottom bar: [disk info] ... [Preview] [Copy]

### Watch tab
Drop zone (monitor folder) + destination list + "+ Add destination".
Note: "Uses your current settings."
Bottom bar: [Start watching] / [Stop watching]

### Health tab
4 selectable cards, each reveals a detail panel:
1. **Bit-rot scan**: folder + [Full scan | Since date]
2. **Verify**: [Two folders | JSON manifest | Certificate] + matching drop zones
3. **PAR2 repair**: folder + warning note + confirmation
4. **Certificate**: [Create | Verify] + matching drop zones

### History tab
Auto-loaded session list, search bar, context menu per row (Export ALE/CSV/PDF, Verify again)

### Settings tab
Sections: General (toggles), Hash, PAR2 slider, Video mode (conditional), Reports, Profiles, Language

---

## PROGRESS PROTOCOL

With `--progress-format machine`, ferr outputs:
```
PROGRESS:<bytes>/<total>|<files>/<total>|<speed>|<filename>
COMPLETE:<files>|<bytes>|<errors>|<manifest_path>
ERROR:<filename>|<type>|<details>
SCAN_PROGRESS:<scanned>/<total>|<file>
SCAN_RESULT:OK|<checked>|<corrupted>
VERIFY_RESULT:OK|<matched>|<mismatched>|<missing>
WATCH_DETECTED:<volume>|<mount>
WATCH_COPY_START:<volume>|<dest>
```

---

## DESIGN SPECS

### CSS variables (light + dark mode)
```css
:root {
    --bg-primary: #ffffff;
    --bg-secondary: #f5f5f5;
    --bg-tertiary: #ebebeb;
    --text-primary: #1a1a1a;
    --text-secondary: #666666;
    --border: rgba(0,0,0,0.12);
    --accent: #2563eb;
    --accent-bg: rgba(37,99,235,0.08);
    --accent-text: #1d4ed8;
    --success: #16a34a;
    --warning: #d97706;
    --danger: #dc2626;
}
@media (prefers-color-scheme: dark) {
    :root {
        --bg-primary: #1e1e1e;
        --bg-secondary: #2a2a2a;
        --bg-tertiary: #333333;
        --text-primary: #e5e5e5;
        --text-secondary: #999999;
        --border: rgba(255,255,255,0.1);
        --accent: #3b82f6;
        --accent-bg: rgba(59,130,246,0.12);
        --accent-text: #60a5fa;
        --success: #22c55e;
        --warning: #f59e0b;
        --danger: #ef4444;
    }
}
```

### Layout: 52px sidebar, 20px content padding, 56px bottom bar
### Radius: 12px cards, 8px buttons/pills
### Drop zones: dashed 1.5px empty, solid blue filled
### No shadows, no gradients, no blur

---

## BUILD PHASES

### Phase 1 — Skeleton
1. Init Tauri v2 project, configure tauri.conf.json + plugins
2. Create full folder structure with all stub files
3. Build index.html shell, main.css with variables + dark mode
4. Build sidebar, topbar, bottombar, tab routing in app.js
5. Build dropzone.js component (folder picker via Tauri dialog)
6. Build copy.js tab (source + dest + mirror)
7. Create Rust commands.rs with pick_folder + basic run_copy
8. Test: app launches, navigate tabs, pick folders

### Phase 2 — Settings & Progress
9. Build settings.js tab with toggles, radios, slider, conditional reveals
10. Build settings persistence with Tauri store plugin
11. Build pills.js, wire to settings
12. Build progress.js parser + overlay
13. Wire run_copy with streaming events
14. Implement Preview + Resume
15. Build volume detection, wire to bottom bar disk info

### Phase 3 — Health & Watch
16. Build healthcard.js + health.js with 4 cards and detail panels
17. Wire scan, verify (3 modes), repair, cert (2 modes) to Rust commands
18. Build watch.js with multi-destination
19. Wire watch start/stop with event streaming
20. Background mode + notifications

### Phase 4 — History & Polish
21. Build history.js — load sessions, search, context menus
22. Wire export ALE/CSV/PDF from history rows
23. Profile management (save/delete/select)
24. Localization (en.json + fr.json)
25. Error handling, dark mode testing, end-to-end test

---

## DEV MODE

The ferr binary won't be available during initial development.
Add `const DEV_MODE = true;` in bridge.js that mocks CLI responses:
- Copy: emit fake PROGRESS lines on a timer
- History: return hardcoded JSON sessions
- Volumes: return fake disk info
- Scan/Verify: return fake results after a delay

The app should look and work perfectly with mocks before connecting the real CLI.

---

## CLI REFERENCE

```
ferr copy <SRC> <DEST> [--dest2 PATH] [--dest3 PATH] [--hash xxhash|sha256]
    [--par2 N] [--resume] [--camera] [--rename TEMPLATE] [--eject] [--dedup]
    [--profile NAME] [--no-preserve-meta] [--no-notify] [--no-pdf]
    [--dry-run] [--quiet] [--progress-format machine]

ferr verify <SRC_OR_MANIFEST> <DEST> [--progress-format machine]
ferr scan <FOLDER> [--since ISO_DATE] [--progress-format machine]
ferr repair <FOLDER>
ferr cert create <SRC> --output <PATH>
ferr cert verify <FERRCERT> <FOLDER>
ferr watch <DIR> --dest <D1> [--dest <D2>] [--hash ...] [--par2 N]
    [--camera] [--eject] [--progress-format machine]
ferr export <MANIFEST> --format <ale|csv> --output <PATH>
ferr report <MANIFEST> --output <PATH>
ferr history list [--json]
ferr history find <QUERY> [--json]
ferr profile save <NAME>
ferr profile list [--json]
```

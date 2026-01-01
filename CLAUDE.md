# CLAUDE.md - AI Assistant Guide for TimeTracker

## Project Overview

TimeTracker is a Windows time-tracking application (similar to ManicTime) built with Tauri 2.0. It automatically monitors and records active window usage, providing timeline views, usage statistics, and browser domain analytics.

## Tech Stack

| Component | Technology |
|-----------|------------|
| Framework | Tauri 2.0 |
| Backend | Rust |
| Frontend | Vanilla TypeScript + Vite |
| Database | SQLite (rusqlite) |
| Windows APIs | windows-rs crate (Window tracking, UI Automation) |
| URL Parsing | url crate |
| Styling | CSS with CSS variables (dark/light mode) |

## Project Structure

```
timetracker/
├── app/                          # Main Tauri application
│   ├── src/                      # Frontend source
│   │   ├── main.ts               # Main TypeScript entry point
│   │   └── styles.css            # Global styles
│   ├── src-tauri/                # Rust backend
│   │   ├── src/
│   │   │   ├── main.rs           # Application entry point
│   │   │   └── lib.rs            # Core logic (window watcher, DB, commands)
│   │   ├── capabilities/         # Tauri capability definitions
│   │   ├── Cargo.toml            # Rust dependencies
│   │   └── tauri.conf.json       # Tauri configuration
│   ├── index.html                # Main HTML template
│   ├── package.json              # Node.js dependencies
│   └── vite.config.ts            # Vite configuration
├── docs/
│   └── TECH_SELECTION.md         # Technology selection rationale
├── .github/workflows/
│   └── build.yml                 # CI/CD pipeline for Windows builds
└── LICENSE                       # MIT License
```

## Development Commands

All commands should be run from the `app/` directory:

```bash
cd app

# Install dependencies
npm install

# Development mode (hot reload)
npm run tauri dev

# Build for production
npm run tauri build

# Frontend only (for UI development)
npm run dev

# Type check and build frontend
npm run build
```

## Architecture

### Backend (Rust - `app/src-tauri/src/lib.rs`)

- **Window Watcher**: Background thread monitoring active windows every second using Windows APIs (`GetForegroundWindow`, `GetWindowText`, `GetModuleBaseName`)
- **Browser URL Extraction**: Uses Windows UI Automation API to read browser address bars and extract domains
- **Database**: SQLite storage in `%LOCALAPPDATA%/timetracker/activities.db`
- **System Tray**: Minimizes to tray, click to restore, context menu for Show/Quit
- **Tauri Commands**: `start_tracking`, `stop_tracking`, `is_tracking`, `get_activities`, `get_app_summary`, `get_domain_summary`

### Browser Domain Aggregation

The application detects browser processes and extracts the current URL from the address bar:

- **Supported Browsers**: Chrome, Edge, Firefox, Brave, Opera, Vivaldi, Internet Explorer
- **URL Extraction**: Uses `IUIAutomation` to find address bar Edit controls
- **Domain Parsing**: Extracts domain from URLs using the `url` crate
- **Aggregation**: Tracks time spent per domain separately from app usage

### Frontend (TypeScript - `app/src/main.ts`)

- **Timeline View**: Chronological list of activities with color-coded apps and domain info
- **App Summary View**: Per-app usage statistics with percentages
- **Domain Summary View**: Per-domain browser usage statistics
- **Auto-refresh**: Updates every 30 seconds when viewing today's data
- **Date Picker**: View historical data by date

### Data Model

```sql
CREATE TABLE activities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    process_name TEXT NOT NULL,      -- e.g., "chrome.exe"
    window_title TEXT NOT NULL,      -- e.g., "Google - Chrome"
    domain TEXT,                     -- e.g., "github.com" (NULL for non-browser apps)
    start_time TEXT NOT NULL,        -- ISO format: YYYY-MM-DDTHH:MM:SS
    end_time TEXT NOT NULL,
    duration_seconds INTEGER NOT NULL
);

-- Indexes for query performance
CREATE INDEX idx_start_time ON activities(start_time);
CREATE INDEX idx_domain ON activities(domain);
```

**Note**: The `domain` column was added via migration. Existing databases are automatically updated with `ALTER TABLE`.

## Key Conventions

### Rust Code

- Use `parking_lot::Mutex` instead of `std::sync::Mutex` for performance
- Windows-specific code is conditionally compiled with `#[cfg(target_os = "windows")]`
- Non-Windows platforms have a stub implementation with demo data for development
- Error handling: Return `Result<T, String>` from Tauri commands
- Date/time: Use `chrono` crate with `Local` timezone
- Browser detection uses a const array of known browser process names
- UI Automation requires COM initialization (`CoInitializeEx`)

### TypeScript Code

- Use Tauri's `invoke()` for calling Rust commands
- Interface definitions mirror Rust structs (`ActivityRecord`, `AppSummary`, `DomainSummary`)
- HTML escaping via `escapeHtml()` function for security
- Time formatting uses Japanese locale (`ja-JP`)
- Domain field is nullable (`domain: string | null`)

### Styling

- CSS variables for theming (dark mode default, light mode via `prefers-color-scheme`)
- CSS Grid layout: Timeline spans full height, App Summary and Domain Summary side by side
- Mobile-responsive with media queries at 900px breakpoint
- Color palette for apps/domains assigned dynamically

## CI/CD

GitHub Actions workflow (`.github/workflows/build.yml`):

- Triggers on version tags (`v*`) or manual dispatch
- Builds Windows MSI and NSIS installers
- Uploads artifacts or creates GitHub releases

## Common Development Tasks

### Adding a New Tauri Command

1. Add function in `app/src-tauri/src/lib.rs`:
   ```rust
   #[tauri::command]
   fn my_command(state: State<Arc<AppState>>) -> Result<T, String> {
       // Implementation
   }
   ```

2. Register in `invoke_handler`:
   ```rust
   .invoke_handler(tauri::generate_handler![
       // ... existing commands
       my_command,
   ])
   ```

3. Call from frontend:
   ```typescript
   const result = await invoke<ReturnType>("my_command", { arg1: value });
   ```

### Modifying the Database Schema

1. Update the `CREATE TABLE` statement in `AppState::new()`
2. Add migration logic with `ALTER TABLE` for existing databases (see domain column example)
3. Update corresponding Rust structs and TypeScript interfaces
4. Add indexes for frequently queried columns

### Adding UI Components

1. Add HTML elements to `app/index.html`
2. Add styles to `app/src/styles.css`
3. Add interactivity in `app/src/main.ts`

### Adding Browser Support

To add support for a new browser:

1. Add the process name to `BROWSER_PROCESSES` array in `windows_watcher` module
2. Test that UI Automation can find the address bar (may need browser-specific logic)

## Dependencies

### Rust (Cargo.toml)

- `tauri` 2.x - Application framework
- `rusqlite` with bundled SQLite
- `chrono` - Date/time handling
- `serde`/`serde_json` - Serialization
- `tokio` - Async runtime
- `parking_lot` - Efficient synchronization
- `dirs` - Platform-specific directories
- `windows` 0.58 (Windows-only) - Windows API bindings
  - `Win32_Foundation`
  - `Win32_UI_WindowsAndMessaging`
  - `Win32_System_Threading`
  - `Win32_System_ProcessStatus`
  - `Win32_UI_Accessibility` - For browser URL extraction
  - `Win32_System_Com` - COM initialization for UI Automation
- `url` 2.x (Windows-only) - URL parsing for domain extraction

### Node.js (package.json)

- `@tauri-apps/api` 2.x - Tauri JavaScript API
- `@tauri-apps/cli` 2.x - Build tools
- `vite` 6.x - Build tool and dev server
- `typescript` 5.6.x

## Platform Notes

- **Target Platform**: Windows 10/11 (requires WebView2, pre-installed)
- **Development**: Works on any platform (non-Windows uses demo stubs with simulated browser activity)
- **Minimum Window Size**: 800x600 pixels
- **Installers**: MSI (enterprise) and NSIS (standard user)

## Troubleshooting

### Common Issues

1. **WebView2 not found**: Install Microsoft Edge WebView2 Runtime
2. **Build fails on Windows**: Ensure Rust and Node.js are installed
3. **Database errors**: Check write permissions to `%LOCALAPPDATA%/timetracker/`
4. **Window tracking not working**: Run as administrator or check process permissions
5. **Browser URL not detected**: Some browsers may use non-standard address bar implementations. Check UI Automation tree with tools like Accessibility Insights.

### Development Tips

- Use `npm run dev` for rapid frontend iteration without rebuilding Rust
- The dev server runs on port 1420 (configured in vite.config.ts)
- Check browser dev tools (F12 in the Tauri window) for frontend debugging
- Rust panics appear in the terminal where `tauri dev` was started
- Non-Windows development shows rotating demo data (Chrome/GitHub, Chrome/Google, VS Code)

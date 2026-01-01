# CLAUDE.md - AI Assistant Guide for TimeTracker

## Project Overview

TimeTracker is a Windows time-tracking application (similar to ManicTime) built with Tauri 2.0. It automatically monitors and records active window usage, providing timeline views and usage statistics.

## Tech Stack

| Component | Technology |
|-----------|------------|
| Framework | Tauri 2.0 |
| Backend | Rust |
| Frontend | Vanilla TypeScript + Vite |
| Database | SQLite (rusqlite) |
| Windows APIs | windows-rs crate |
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
- **Database**: SQLite storage in `%LOCALAPPDATA%/timetracker/activities.db`
- **System Tray**: Minimizes to tray, click to restore, context menu for Show/Quit
- **Tauri Commands**: `start_tracking`, `stop_tracking`, `is_tracking`, `get_activities`, `get_app_summary`

### Frontend (TypeScript - `app/src/main.ts`)

- **Timeline View**: Chronological list of activities with color-coded apps
- **Summary View**: Per-app usage statistics with percentages
- **Auto-refresh**: Updates every 30 seconds when viewing today's data
- **Date Picker**: View historical data by date

### Data Model

```sql
CREATE TABLE activities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    process_name TEXT NOT NULL,      -- e.g., "chrome.exe"
    window_title TEXT NOT NULL,       -- e.g., "Google - Chrome"
    start_time TEXT NOT NULL,         -- ISO format: YYYY-MM-DDTHH:MM:SS
    end_time TEXT NOT NULL,
    duration_seconds INTEGER NOT NULL
);
```

## Key Conventions

### Rust Code

- Use `parking_lot::Mutex` instead of `std::sync::Mutex` for performance
- Windows-specific code is conditionally compiled with `#[cfg(target_os = "windows")]`
- Non-Windows platforms have a stub implementation for development
- Error handling: Return `Result<T, String>` from Tauri commands
- Date/time: Use `chrono` crate with `Local` timezone

### TypeScript Code

- Use Tauri's `invoke()` for calling Rust commands
- Interface definitions mirror Rust structs (`ActivityRecord`, `AppSummary`)
- HTML escaping via `escapeHtml()` function for security
- Time formatting uses Japanese locale (`ja-JP`)

### Styling

- CSS variables for theming (dark mode default, light mode via `prefers-color-scheme`)
- Mobile-responsive with media queries at 900px breakpoint
- Color palette for apps assigned dynamically

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
2. Consider migration logic for existing databases
3. Update corresponding Rust structs and TypeScript interfaces

### Adding UI Components

1. Add HTML elements to `app/index.html`
2. Add styles to `app/src/styles.css`
3. Add interactivity in `app/src/main.ts`

## Dependencies

### Rust (Cargo.toml)

- `tauri` 2.x - Application framework
- `rusqlite` with bundled SQLite
- `chrono` - Date/time handling
- `serde`/`serde_json` - Serialization
- `tokio` - Async runtime
- `parking_lot` - Efficient synchronization
- `windows` 0.58 (Windows-only) - Windows API bindings

### Node.js (package.json)

- `@tauri-apps/api` 2.x - Tauri JavaScript API
- `@tauri-apps/cli` 2.x - Build tools
- `vite` 6.x - Build tool and dev server
- `typescript` 5.6.x

## Platform Notes

- **Target Platform**: Windows 10/11 (requires WebView2, pre-installed)
- **Development**: Works on any platform (non-Windows uses demo stubs)
- **Minimum Window Size**: 800x600 pixels
- **Installers**: MSI (enterprise) and NSIS (standard user)

## Troubleshooting

### Common Issues

1. **WebView2 not found**: Install Microsoft Edge WebView2 Runtime
2. **Build fails on Windows**: Ensure Rust and Node.js are installed
3. **Database errors**: Check write permissions to `%LOCALAPPDATA%/timetracker/`
4. **Window tracking not working**: Run as administrator or check process permissions

### Development Tips

- Use `npm run dev` for rapid frontend iteration without rebuilding Rust
- The dev server runs on port 1420 (configured in vite.config.ts)
- Check browser dev tools (F12 in the Tauri window) for frontend debugging
- Rust panics appear in the terminal where `tauri dev` was started

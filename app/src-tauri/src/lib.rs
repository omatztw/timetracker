use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle, Manager, State,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    WindowEvent,
};

#[cfg(target_os = "windows")]
mod windows_watcher {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;

    pub fn get_active_window_info() -> Option<(String, String)> {
        unsafe {
            let hwnd: HWND = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }

            // Get window title
            let mut title_buf = [0u16; 512];
            let title_len = GetWindowTextW(hwnd, &mut title_buf);
            let title = if title_len > 0 {
                String::from_utf16_lossy(&title_buf[..title_len as usize])
            } else {
                String::new()
            };

            // Get process name
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            let process_name = if process_id != 0 {
                if let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
                    let mut name_buf = [0u16; 256];
                    let name_len = GetModuleBaseNameW(handle, None, &mut name_buf);
                    if name_len > 0 {
                        String::from_utf16_lossy(&name_buf[..name_len as usize])
                    } else {
                        String::from("Unknown")
                    }
                } else {
                    String::from("Unknown")
                }
            } else {
                String::from("Unknown")
            };

            Some((process_name, title))
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod windows_watcher {
    pub fn get_active_window_info() -> Option<(String, String)> {
        // Stub for non-Windows platforms (development only)
        Some((String::from("DemoApp.exe"), String::from("Demo Window - Development Mode")))
    }
}

use windows_watcher::get_active_window_info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRecord {
    pub id: i64,
    pub process_name: String,
    pub window_title: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSummary {
    pub process_name: String,
    pub total_seconds: i64,
    pub percentage: f64,
}

pub struct AppState {
    db: Mutex<Connection>,
    is_tracking: Mutex<bool>,
}

impl AppState {
    fn new() -> Result<Self, rusqlite::Error> {
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("timetracker")
            .join("activities.db");

        std::fs::create_dir_all(db_path.parent().unwrap()).ok();

        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS activities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                process_name TEXT NOT NULL,
                window_title TEXT NOT NULL,
                start_time TEXT NOT NULL,
                end_time TEXT NOT NULL,
                duration_seconds INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_start_time ON activities(start_time)",
            [],
        )?;

        Ok(Self {
            db: Mutex::new(conn),
            is_tracking: Mutex::new(false),
        })
    }
}

#[tauri::command]
fn start_tracking(state: State<Arc<AppState>>) -> Result<(), String> {
    let mut is_tracking = state.is_tracking.lock();
    *is_tracking = true;
    Ok(())
}

#[tauri::command]
fn stop_tracking(state: State<Arc<AppState>>) -> Result<(), String> {
    let mut is_tracking = state.is_tracking.lock();
    *is_tracking = false;
    Ok(())
}

#[tauri::command]
fn is_tracking(state: State<Arc<AppState>>) -> bool {
    *state.is_tracking.lock()
}

#[tauri::command]
fn get_activities(state: State<Arc<AppState>>, date: String) -> Result<Vec<ActivityRecord>, String> {
    let db = state.db.lock();
    let start_of_day = format!("{}T00:00:00", date);
    let end_of_day = format!("{}T23:59:59", date);

    let mut stmt = db
        .prepare(
            "SELECT id, process_name, window_title, start_time, end_time, duration_seconds
             FROM activities
             WHERE start_time >= ?1 AND start_time <= ?2
             ORDER BY start_time ASC",
        )
        .map_err(|e| e.to_string())?;

    let records = stmt
        .query_map(params![start_of_day, end_of_day], |row| {
            Ok(ActivityRecord {
                id: row.get(0)?,
                process_name: row.get(1)?,
                window_title: row.get(2)?,
                start_time: row.get(3)?,
                end_time: row.get(4)?,
                duration_seconds: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(records)
}

#[tauri::command]
fn get_app_summary(state: State<Arc<AppState>>, date: String) -> Result<Vec<AppSummary>, String> {
    let db = state.db.lock();
    let start_of_day = format!("{}T00:00:00", date);
    let end_of_day = format!("{}T23:59:59", date);

    let mut stmt = db
        .prepare(
            "SELECT process_name, SUM(duration_seconds) as total
             FROM activities
             WHERE start_time >= ?1 AND start_time <= ?2
             GROUP BY process_name
             ORDER BY total DESC",
        )
        .map_err(|e| e.to_string())?;

    let summaries: Vec<(String, i64)> = stmt
        .query_map(params![start_of_day, end_of_day], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let total_seconds: i64 = summaries.iter().map(|(_, s)| s).sum();

    let result = summaries
        .into_iter()
        .map(|(name, secs)| AppSummary {
            process_name: name,
            total_seconds: secs,
            percentage: if total_seconds > 0 {
                (secs as f64 / total_seconds as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect();

    Ok(result)
}

fn start_watcher_thread(state: Arc<AppState>) {
    thread::spawn(move || {
        let mut last_process = String::new();
        let mut last_title = String::new();
        let mut activity_start: Option<DateTime<Local>> = None;

        loop {
            thread::sleep(Duration::from_secs(1));

            if !*state.is_tracking.lock() {
                // Save current activity before pausing
                if let Some(start) = activity_start.take() {
                    save_activity(&state, &last_process, &last_title, start);
                }
                last_process.clear();
                last_title.clear();
                continue;
            }

            if let Some((process_name, window_title)) = get_active_window_info() {
                let changed = process_name != last_process || window_title != last_title;

                if changed {
                    // Save previous activity
                    if let Some(start) = activity_start.take() {
                        save_activity(&state, &last_process, &last_title, start);
                    }

                    // Start new activity
                    last_process = process_name;
                    last_title = window_title;
                    activity_start = Some(Local::now());
                }
            }
        }
    });
}

fn save_activity(state: &Arc<AppState>, process_name: &str, window_title: &str, start: DateTime<Local>) {
    if process_name.is_empty() {
        return;
    }

    let end = Local::now();
    let duration = (end - start).num_seconds();

    if duration < 1 {
        return;
    }

    let db = state.db.lock();
    let _ = db.execute(
        "INSERT INTO activities (process_name, window_title, start_time, end_time, duration_seconds)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            process_name,
            window_title,
            start.format("%Y-%m-%dT%H:%M:%S").to_string(),
            end.format("%Y-%m-%dT%H:%M:%S").to_string(),
            duration,
        ],
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState::new().expect("Failed to initialize database"));
    let watcher_state = app_state.clone();

    // Start tracking by default
    *app_state.is_tracking.lock() = true;

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .setup(move |app| {
            // Start the background watcher
            start_watcher_thread(watcher_state);

            // Setup system tray
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("TimeTracker - Running")
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_tracking,
            stop_tracking,
            is_tracking,
            get_activities,
            get_app_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

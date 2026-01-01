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

pub mod plugins;

use plugins::{PluginManager, traits::{ActivityInfo, SyncResult}, config::IntegrationsConfig};

#[cfg(target_os = "windows")]
mod windows_watcher {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationCondition,
        UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_NamePropertyId,
        UIA_ValueValuePropertyId, TreeScope_Subtree,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoCreateInstance, CLSCTX_ALL, COINIT_MULTITHREADED};
    use url::Url;

    // Browser process names
    const BROWSER_PROCESSES: &[&str] = &[
        "chrome.exe",
        "msedge.exe",
        "firefox.exe",
        "brave.exe",
        "opera.exe",
        "vivaldi.exe",
        "iexplore.exe",
    ];

    pub fn is_browser(process_name: &str) -> bool {
        let lower = process_name.to_lowercase();
        BROWSER_PROCESSES.iter().any(|b| lower == *b)
    }

    pub fn extract_domain(url_str: &str) -> Option<String> {
        if let Ok(url) = Url::parse(url_str) {
            url.host_str().map(|h| h.to_string())
        } else {
            None
        }
    }

    pub fn get_browser_url(hwnd: HWND) -> Option<String> {
        unsafe {
            // Initialize COM
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            // Create UI Automation instance
            let automation: IUIAutomation = match CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) {
                Ok(a) => a,
                Err(_) => return None,
            };

            // Get element from window handle
            let element: IUIAutomationElement = match automation.ElementFromHandle(hwnd) {
                Ok(e) => e,
                Err(_) => return None,
            };

            // Create condition to find Edit controls (address bar)
            let edit_condition: IUIAutomationCondition = match automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &windows::core::VARIANT::from(UIA_EditControlTypeId.0),
            ) {
                Ok(c) => c,
                Err(_) => return None,
            };

            // Find all Edit elements
            let elements = match element.FindAll(TreeScope_Subtree, &edit_condition) {
                Ok(e) => e,
                Err(_) => return None,
            };

            let count = elements.Length().unwrap_or(0);

            for i in 0..count {
                if let Ok(elem) = elements.GetElement(i) {
                    // Check if this is the address bar by looking at the name property
                    if let Ok(name) = elem.GetCurrentPropertyValue(UIA_NamePropertyId) {
                        let name_str = name.to_string().to_lowercase();
                        // Common address bar identifiers
                        if name_str.contains("address") ||
                           name_str.contains("url") ||
                           name_str.contains("アドレス") ||
                           name_str.contains("location") {
                            // Get the value (URL)
                            if let Ok(value) = elem.GetCurrentPropertyValue(UIA_ValueValuePropertyId) {
                                let url_str = value.to_string();
                                if url_str.starts_with("http://") || url_str.starts_with("https://") {
                                    return Some(url_str);
                                }
                            }
                        }
                    }
                }
            }

            // Fallback: try to find any edit control with a URL-like value
            for i in 0..count {
                if let Ok(elem) = elements.GetElement(i) {
                    if let Ok(value) = elem.GetCurrentPropertyValue(UIA_ValueValuePropertyId) {
                        let url_str = value.to_string();
                        if url_str.starts_with("http://") || url_str.starts_with("https://") {
                            return Some(url_str);
                        }
                    }
                }
            }

            None
        }
    }

    pub fn get_active_window_info() -> Option<(String, String, Option<String>)> {
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

            // Get domain if it's a browser
            let domain = if is_browser(&process_name) {
                get_browser_url(hwnd).and_then(|url| extract_domain(&url))
            } else {
                None
            };

            Some((process_name, title, domain))
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod windows_watcher {
    pub fn get_active_window_info() -> Option<(String, String, Option<String>)> {
        // Stub for non-Windows platforms (development only)
        // Simulate browser with domain for testing
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Alternate between browser and non-browser for demo
        if count % 3 == 0 {
            Some((
                String::from("chrome.exe"),
                String::from("GitHub - Demo Page"),
                Some(String::from("github.com")),
            ))
        } else if count % 3 == 1 {
            Some((
                String::from("chrome.exe"),
                String::from("Google Search"),
                Some(String::from("google.com")),
            ))
        } else {
            Some((
                String::from("Code.exe"),
                String::from("lib.rs - timetracker"),
                None,
            ))
        }
    }
}

use windows_watcher::get_active_window_info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRecord {
    pub id: i64,
    pub process_name: String,
    pub window_title: String,
    pub domain: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSummary {
    pub domain: String,
    pub total_seconds: i64,
    pub percentage: f64,
}

pub struct AppState {
    db: Mutex<Connection>,
    is_tracking: Mutex<bool>,
    plugin_manager: PluginManager,
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
                domain TEXT,
                start_time TEXT NOT NULL,
                end_time TEXT NOT NULL,
                duration_seconds INTEGER NOT NULL
            )",
            [],
        )?;

        // Add domain column if it doesn't exist (migration for existing databases)
        let _ = conn.execute("ALTER TABLE activities ADD COLUMN domain TEXT", []);

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_start_time ON activities(start_time)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_domain ON activities(domain)",
            [],
        )?;

        // プラグインマネージャーを初期化
        let plugin_manager = PluginManager::new();
        if let Err(e) = plugin_manager.load_from_config() {
            eprintln!("Failed to load plugins: {}", e);
        }

        Ok(Self {
            db: Mutex::new(conn),
            is_tracking: Mutex::new(false),
            plugin_manager,
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
            "SELECT id, process_name, window_title, domain, start_time, end_time, duration_seconds
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
                domain: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5)?,
                duration_seconds: row.get(6)?,
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

#[tauri::command]
fn get_domain_summary(state: State<Arc<AppState>>, date: String) -> Result<Vec<DomainSummary>, String> {
    let db = state.db.lock();
    let start_of_day = format!("{}T00:00:00", date);
    let end_of_day = format!("{}T23:59:59", date);

    let mut stmt = db
        .prepare(
            "SELECT domain, SUM(duration_seconds) as total
             FROM activities
             WHERE start_time >= ?1 AND start_time <= ?2 AND domain IS NOT NULL AND domain != ''
             GROUP BY domain
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
        .map(|(domain, secs)| DomainSummary {
            domain,
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

// ========== プラグイン関連コマンド ==========

/// プラグイン一覧を取得
#[tauri::command]
fn get_plugins(state: State<Arc<AppState>>) -> Vec<String> {
    state.plugin_manager.list_plugins()
}

/// プラグイン設定を再読み込み
#[tauri::command]
fn reload_plugins(state: State<Arc<AppState>>) -> Result<(), String> {
    state.plugin_manager.load_from_config()
}

/// サンプル設定ファイルを作成
#[tauri::command]
fn create_sample_plugin_config() -> Result<String, String> {
    plugins::create_sample_config()?;
    Ok(IntegrationsConfig::config_path().to_string_lossy().to_string())
}

/// 設定ファイルのパスを取得
#[tauri::command]
fn get_plugin_config_path() -> String {
    IntegrationsConfig::config_path().to_string_lossy().to_string()
}

/// アクティビティからチケットIDを抽出
#[tauri::command]
fn extract_ticket_ids(
    state: State<Arc<AppState>>,
    activity_id: i64,
) -> Result<Vec<(String, String)>, String> {
    let db = state.db.lock();

    let mut stmt = db
        .prepare(
            "SELECT id, process_name, window_title, domain, start_time, end_time, duration_seconds
             FROM activities WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let activity = stmt
        .query_row(params![activity_id], |row| {
            Ok(ActivityInfo {
                id: row.get(0)?,
                process_name: row.get(1)?,
                window_title: row.get(2)?,
                domain: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5)?,
                duration_seconds: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    Ok(state.plugin_manager.extract_all_ticket_ids(&activity))
}

/// 作業時間を外部サービスに同期
#[tauri::command]
async fn sync_time_entry(
    state: State<'_, Arc<AppState>>,
    plugin_name: String,
    activity_id: i64,
    ticket_id: String,
) -> Result<SyncResult, String> {
    let activity = {
        let db = state.db.lock();

        let mut stmt = db
            .prepare(
                "SELECT id, process_name, window_title, domain, start_time, end_time, duration_seconds
                 FROM activities WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;

        stmt.query_row(params![activity_id], |row| {
            Ok(ActivityInfo {
                id: row.get(0)?,
                process_name: row.get(1)?,
                window_title: row.get(2)?,
                domain: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5)?,
                duration_seconds: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
    };

    state
        .plugin_manager
        .sync_time_entry(&plugin_name, &activity, &ticket_id)
        .await
}

/// プラグインの接続テスト
#[tauri::command]
async fn test_plugin_connection(
    state: State<'_, Arc<AppState>>,
    plugin_name: String,
) -> Result<bool, String> {
    state.plugin_manager.test_connection(&plugin_name).await
}

fn start_watcher_thread(state: Arc<AppState>) {
    thread::spawn(move || {
        let mut last_process = String::new();
        let mut last_title = String::new();
        let mut last_domain: Option<String> = None;
        let mut activity_start: Option<DateTime<Local>> = None;

        loop {
            thread::sleep(Duration::from_secs(1));

            if !*state.is_tracking.lock() {
                // Save current activity before pausing
                if let Some(start) = activity_start.take() {
                    save_activity(&state, &last_process, &last_title, last_domain.as_deref(), start);
                }
                last_process.clear();
                last_title.clear();
                last_domain = None;
                continue;
            }

            if let Some((process_name, window_title, domain)) = get_active_window_info() {
                let changed = process_name != last_process || window_title != last_title || domain != last_domain;

                if changed {
                    // Save previous activity
                    if let Some(start) = activity_start.take() {
                        save_activity(&state, &last_process, &last_title, last_domain.as_deref(), start);
                    }

                    // Start new activity
                    last_process = process_name;
                    last_title = window_title;
                    last_domain = domain;
                    activity_start = Some(Local::now());
                }
            }
        }
    });
}

fn save_activity(state: &Arc<AppState>, process_name: &str, window_title: &str, domain: Option<&str>, start: DateTime<Local>) {
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
        "INSERT INTO activities (process_name, window_title, domain, start_time, end_time, duration_seconds)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            process_name,
            window_title,
            domain,
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
            get_domain_summary,
            get_plugins,
            reload_plugins,
            create_sample_plugin_config,
            get_plugin_config_path,
            extract_ticket_ids,
            sync_time_entry,
            test_plugin_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

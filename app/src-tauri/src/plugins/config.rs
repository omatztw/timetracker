use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// チケットID抽出ルール
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionRule {
    /// 正規表現パターン（キャプチャグループでIDを抽出）
    pub pattern: String,
    /// 抽出元: "window_title" | "process_name" | "domain"
    pub source: String,
}

/// Redmine固有設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedmineConfig {
    pub url: String,
    pub api_key: String,
    #[serde(default)]
    pub default_activity_id: Option<i64>,
    #[serde(default)]
    pub rules: Vec<ExtractionRule>,
}

/// プラグイン設定（汎用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IntegrationConfig {
    #[serde(rename = "redmine")]
    Redmine(RedmineConfig),
}

/// 個別の連携設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationEntry {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(flatten)]
    pub config: IntegrationConfig,
}

fn default_enabled() -> bool {
    true
}

/// アップロード設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    /// アップロード先サーバーURL (e.g., "https://timetracker.example.com/api")
    pub server_url: String,
    /// アップロードを有効にするかどうか
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 自動アップロードを有効にするかどうか
    #[serde(default)]
    pub auto_upload: bool,
    /// 自動アップロード間隔（分）
    #[serde(default = "default_upload_interval")]
    pub auto_upload_interval_minutes: u32,
}

fn default_upload_interval() -> u32 {
    60 // デフォルト1時間ごと
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            enabled: false,
            auto_upload: false,
            auto_upload_interval_minutes: default_upload_interval(),
        }
    }
}

/// 全体設定ファイル
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntegrationsConfig {
    #[serde(default)]
    pub integrations: Vec<IntegrationEntry>,
    /// データアップロード設定
    #[serde(default)]
    pub upload: Option<UploadConfig>,
}

impl IntegrationsConfig {
    /// 設定ファイルのパスを取得
    pub fn config_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("timetracker")
            .join("integrations.toml")
    }

    /// 設定ファイルを読み込む
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Failed to parse integrations config: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read integrations config: {}", e);
                }
            }
        }
        Self::default()
    }

    /// 設定ファイルを保存
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// サンプル設定を生成
    pub fn create_sample() -> Self {
        Self {
            integrations: vec![IntegrationEntry {
                name: "my-redmine".to_string(),
                enabled: false,
                config: IntegrationConfig::Redmine(RedmineConfig {
                    url: "https://redmine.example.com".to_string(),
                    api_key: "your-api-key-here".to_string(),
                    default_activity_id: Some(9),
                    rules: vec![
                        ExtractionRule {
                            pattern: r"#(\d+)".to_string(),
                            source: "window_title".to_string(),
                        },
                        ExtractionRule {
                            pattern: r"Issue (\d+)".to_string(),
                            source: "window_title".to_string(),
                        },
                    ],
                }),
            }],
            upload: Some(UploadConfig {
                server_url: "https://timetracker.example.com/api/upload".to_string(),
                enabled: false,
                auto_upload: false,
                auto_upload_interval_minutes: 60,
            }),
        }
    }
}

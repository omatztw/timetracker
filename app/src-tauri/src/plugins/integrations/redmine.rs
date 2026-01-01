use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::plugins::config::{ExtractionRule, RedmineConfig};
use crate::plugins::traits::{ActivityInfo, ExternalIntegration, SyncResult};

/// Redmine API: タイムエントリ作成リクエスト
#[derive(Debug, Serialize)]
struct TimeEntryRequest {
    time_entry: TimeEntryData,
}

#[derive(Debug, Serialize)]
struct TimeEntryData {
    issue_id: i64,
    hours: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_id: Option<i64>,
    comments: String,
    spent_on: String,
}

/// Redmine API: タイムエントリ作成レスポンス
#[derive(Debug, Deserialize)]
struct TimeEntryResponse {
    time_entry: TimeEntryInfo,
}

#[derive(Debug, Deserialize)]
struct TimeEntryInfo {
    id: i64,
}

/// Redmine API: ユーザー情報（接続テスト用）
#[derive(Debug, Deserialize)]
struct CurrentUserResponse {
    user: UserInfo,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    id: i64,
    login: String,
}

/// Redmine連携プラグイン
pub struct RedmineIntegration {
    name: String,
    enabled: bool,
    config: RedmineConfig,
    client: Client,
    rules: Vec<(Regex, String)>,
}

impl RedmineIntegration {
    pub fn new(name: String, enabled: bool, config: RedmineConfig) -> Result<Self, String> {
        let client = Client::new();

        // 抽出ルールをコンパイル
        let rules = config
            .rules
            .iter()
            .filter_map(|rule| {
                Regex::new(&rule.pattern)
                    .ok()
                    .map(|re| (re, rule.source.clone()))
            })
            .collect();

        Ok(Self {
            name,
            enabled,
            config,
            client,
            rules,
        })
    }

    fn get_source_text<'a>(&self, activity: &'a ActivityInfo, source: &str) -> &'a str {
        match source {
            "window_title" => &activity.window_title,
            "process_name" => &activity.process_name,
            "domain" => activity.domain.as_deref().unwrap_or(""),
            _ => &activity.window_title,
        }
    }
}

#[async_trait]
impl ExternalIntegration for RedmineIntegration {
    fn name(&self) -> &str {
        &self.name
    }

    fn display_name(&self) -> &str {
        "Redmine"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn extract_ticket_id(&self, activity: &ActivityInfo) -> Option<String> {
        for (regex, source) in &self.rules {
            let text = self.get_source_text(activity, source);
            if let Some(captures) = regex.captures(text) {
                if let Some(id) = captures.get(1) {
                    return Some(id.as_str().to_string());
                }
            }
        }
        None
    }

    async fn sync_time_entry(
        &self,
        activity: &ActivityInfo,
        ticket_id: &str,
    ) -> Result<SyncResult, String> {
        let issue_id: i64 = ticket_id
            .parse()
            .map_err(|_| format!("Invalid ticket ID: {}", ticket_id))?;

        // 時間を時間単位に変換（秒 → 時）
        let hours = activity.duration_seconds as f64 / 3600.0;

        // 日付を抽出（YYYY-MM-DD形式）
        let spent_on = activity.start_time.split('T').next().unwrap_or("").to_string();

        let request = TimeEntryRequest {
            time_entry: TimeEntryData {
                issue_id,
                hours,
                activity_id: self.config.default_activity_id,
                comments: format!(
                    "{} - {}",
                    activity.process_name, activity.window_title
                ),
                spent_on,
            },
        };

        let url = format!("{}/time_entries.json", self.config.url.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .header("X-Redmine-API-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if response.status().is_success() {
            let result: TimeEntryResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            Ok(SyncResult {
                success: true,
                message: format!("Created time entry #{}", result.time_entry.id),
                external_id: Some(result.time_entry.id.to_string()),
            })
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Redmine API error ({}): {}", status, body))
        }
    }

    async fn test_connection(&self) -> Result<bool, String> {
        let url = format!(
            "{}/users/current.json",
            self.config.url.trim_end_matches('/')
        );

        let response = self
            .client
            .get(&url)
            .header("X-Redmine-API-Key", &self.config.api_key)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if response.status().is_success() {
            let result: CurrentUserResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            println!(
                "Connected to Redmine as: {} (id: {})",
                result.user.login, result.user.id
            );
            Ok(true)
        } else {
            Err(format!("Authentication failed: {}", response.status()))
        }
    }
}

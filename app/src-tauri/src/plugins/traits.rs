use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// アクティビティ記録（プラグインに渡すデータ）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityInfo {
    pub id: i64,
    pub process_name: String,
    pub window_title: String,
    pub domain: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: i64,
}

/// 同期結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub success: bool,
    pub message: String,
    pub external_id: Option<String>,
}

/// 外部連携プラグインのトレイト
#[async_trait]
pub trait ExternalIntegration: Send + Sync {
    /// プラグイン名を返す
    fn name(&self) -> &str;

    /// プラグインの表示名を返す
    fn display_name(&self) -> &str;

    /// プラグインが有効かどうか
    fn is_enabled(&self) -> bool;

    /// アクティビティからチケット/タスクIDを抽出する
    fn extract_ticket_id(&self, activity: &ActivityInfo) -> Option<String>;

    /// 作業時間を外部サービスに同期する
    async fn sync_time_entry(
        &self,
        activity: &ActivityInfo,
        ticket_id: &str,
    ) -> Result<SyncResult, String>;

    /// 接続テスト
    async fn test_connection(&self) -> Result<bool, String>;
}

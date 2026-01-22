pub mod config;
pub mod integrations;
pub mod traits;

use parking_lot::RwLock;
use std::sync::Arc;

use config::{IntegrationConfig, IntegrationsConfig, UploadConfig};
use integrations::RedmineIntegration;
use traits::{ActivityInfo, ExternalIntegration, SyncResult};

pub use config::UploadConfig;

/// プラグインマネージャー
pub struct PluginManager {
    plugins: RwLock<Vec<Arc<dyn ExternalIntegration>>>,
}

impl PluginManager {
    /// 新しいプラグインマネージャーを作成
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(Vec::new()),
        }
    }

    /// 設定ファイルからプラグインを読み込む
    pub fn load_from_config(&self) -> Result<(), String> {
        let config = IntegrationsConfig::load();
        let mut plugins = self.plugins.write();
        plugins.clear();

        for entry in config.integrations {
            if !entry.enabled {
                continue;
            }

            let plugin: Arc<dyn ExternalIntegration> =
                match entry.config {
                    IntegrationConfig::Redmine(redmine_config) => Arc::new(
                        RedmineIntegration::new(entry.name, entry.enabled, redmine_config)?,
                    ),
                };

            plugins.push(plugin);
        }

        Ok(())
    }

    /// 有効なプラグイン一覧を取得
    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins
            .read()
            .iter()
            .map(|p| p.name().to_string())
            .collect()
    }

    /// 指定したプラグインを取得
    pub fn get_plugin(&self, name: &str) -> Option<Arc<dyn ExternalIntegration>> {
        self.plugins
            .read()
            .iter()
            .find(|p| p.name() == name)
            .cloned()
    }

    /// アクティビティからチケットIDを抽出（最初にマッチしたプラグインの結果を返す）
    pub fn extract_ticket_id(&self, activity: &ActivityInfo) -> Option<(String, String)> {
        for plugin in self.plugins.read().iter() {
            if let Some(ticket_id) = plugin.extract_ticket_id(activity) {
                return Some((plugin.name().to_string(), ticket_id));
            }
        }
        None
    }

    /// 全プラグインで抽出を試行し、結果を返す
    pub fn extract_all_ticket_ids(&self, activity: &ActivityInfo) -> Vec<(String, String)> {
        let mut results = Vec::new();
        for plugin in self.plugins.read().iter() {
            if let Some(ticket_id) = plugin.extract_ticket_id(activity) {
                results.push((plugin.name().to_string(), ticket_id));
            }
        }
        results
    }

    /// 作業時間を同期
    pub async fn sync_time_entry(
        &self,
        plugin_name: &str,
        activity: &ActivityInfo,
        ticket_id: &str,
    ) -> Result<SyncResult, String> {
        let plugin = self
            .get_plugin(plugin_name)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_name))?;

        plugin.sync_time_entry(activity, ticket_id).await
    }

    /// 接続テスト
    pub async fn test_connection(&self, plugin_name: &str) -> Result<bool, String> {
        let plugin = self
            .get_plugin(plugin_name)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_name))?;

        plugin.test_connection().await
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 設定ファイルのサンプルを作成
pub fn create_sample_config() -> Result<(), String> {
    let config = IntegrationsConfig::create_sample();
    config.save()
}

/// アップロード設定を取得
pub fn get_upload_config() -> Option<UploadConfig> {
    let config = IntegrationsConfig::load();
    config.upload
}

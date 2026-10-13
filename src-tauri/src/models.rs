use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub destination: String,
    pub separate_folders: bool,
    pub concurrency: usize,
    pub theme: String,
    pub notifications: bool,
    pub skip_duplicates: bool,
    pub auto_connect: bool,
    pub allowed_chat_ids: Vec<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            destination: String::new(),
            separate_folders: false,
            concurrency: 2,
            theme: "system".into(),
            notifications: false,
            skip_duplicates: true,
            auto_connect: false,
            allowed_chat_ids: Vec::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=6).contains(&self.concurrency) {
            return Err("Параллельность: от 1 до 6.".into());
        }
        if !["light", "dark", "system"].contains(&self.theme.as_str()) {
            return Err("Неизвестная тема.".into());
        }
        if self.allowed_chat_ids.len() > 100 {
            return Err("Не более 100 разрешённых чатов.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub id: i64,
    pub bot_id: i64,
    pub update_id: i64,
    pub chat_id: i64,
    pub message_id: i64,
    pub media_group_id: Option<String>,
    pub file_id: String,
    pub file_unique_id: String,
    pub filename: String,
    pub media_type: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub speed: f64,
    pub status: String,
    pub destination: String,
    pub final_path: Option<String>,
    pub temp_path: Option<String>,
    pub sha256: Option<String>,
    pub error: Option<String>,
    pub attempts: u32,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub running: bool,
    pub state: String,
    pub message: String,
    pub bot_username: Option<String>,
    pub bot_id: Option<i64>,
    pub last_success: Option<i64>,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            running: false,
            state: "not_configured".into(),
            message: "Добавьте токен бота в настройках.".into(),
            bot_username: None,
            bot_id: None,
            last_success: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub queued: u64,
    pub downloading: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: i64,
    pub time: i64,
    pub level: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Settings,
    pub connection: Connection,
    pub counts: Counts,
    pub downloads: Vec<Download>,
    pub activity: Vec<Activity>,
    pub token_configured: bool,
    pub version: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryQuery {
    pub search: String,
    pub status: String,
    pub media_type: String,
    pub page: u32,
}

#[derive(Serialize)]
pub struct HistoryPage {
    pub items: Vec<Download>,
    pub total: u64,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

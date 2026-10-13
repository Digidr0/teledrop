use crate::{filesystem::sanitize_filename, models::*, security::transport_message};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::time::Duration;

pub const HOSTED_LIMIT: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub message: String,
    pub retryable: bool,
    pub auth: bool,
    pub retry_after: Option<u64>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApiError {}

impl ApiError {
    pub fn network(e: reqwest::Error) -> Self {
        Self {
            message: transport_message(&e).into(),
            retryable: true,
            auth: false,
            retry_after: None,
        }
    }
    pub fn permanent(message: &str) -> Self {
        Self {
            message: message.into(),
            retryable: false,
            auth: false,
            retry_after: None,
        }
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    ok: bool,
    result: Option<T>,
    error_code: Option<u16>,
    parameters: Option<Parameters>,
}
#[derive(Deserialize)]
struct Parameters {
    retry_after: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bot {
    pub id: i64,
    pub username: Option<String>,
    pub is_bot: bool,
}
#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub channel_post: Option<Message>,
}
#[derive(Debug, Deserialize)]
pub struct Message {
    pub text: Option<String>,
    pub message_id: i64,
    pub chat: Chat,
    pub media_group_id: Option<String>,
    pub photo: Option<Vec<Media>>,
    pub video: Option<Media>,
    pub document: Option<Media>,
}
#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
}
#[derive(Debug, Deserialize)]
pub struct Media {
    pub file_id: String,
    pub file_unique_id: String,
    pub file_size: Option<u64>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
}
#[derive(Deserialize)]
pub struct RemoteFile {
    pub file_path: Option<String>,
    pub file_size: Option<u64>,
}

#[derive(Clone)]
pub struct Telegram {
    http: reqwest::Client,
    token: std::sync::Arc<SecretString>,
    base: String,
}

impl Telegram {
    #[cfg(test)]
    pub fn for_test(base: String) -> Self {
        let mut api = Self::new(SecretString::from("123:TEST_ONLY".to_string())).unwrap();
        api.base = base;
        api
    }
    pub fn new(token: SecretString) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(65))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("TeleDrop/0.1")
            .build()?;
        Ok(Self {
            http,
            token: std::sync::Arc::new(token),
            base: "https://api.telegram.org".into(),
        })
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        body: serde_json::Value,
    ) -> Result<T, ApiError> {
        let url = format!("{}/bot{}/{method}", self.base, self.token.expose_secret());
        let response = self
            .http
            .post(url)
            .timeout(Duration::from_secs(60))
            .json(&body)
            .send()
            .await
            .map_err(ApiError::network)?;
        let status = response.status().as_u16();
        if status >= 500 {
            return Err(ApiError {
                message: "Telegram временно недоступен.".into(),
                retryable: true,
                auth: false,
                retry_after: None,
            });
        }
        let raw = response
            .json::<Envelope<T>>()
            .await
            .map_err(ApiError::network)?;
        if raw.ok {
            return raw
                .result
                .ok_or_else(|| ApiError::permanent("Неполный ответ Telegram."));
        }
        let code = raw.error_code.unwrap_or(status);
        let message = match code {
            401 | 404 => "Токен недействителен или отозван.",
            409 => {
                "Бот занят другим poller или webhook. Остановите другой клиент; webhook не удалён."
            }
            429 => "Telegram ограничил частоту запросов.",
            400 if method == "getFile" => {
                "Telegram не предоставил файл. Проверьте лимит hosted API 20 MB."
            }
            403 => "Доступ к Telegram запрещён.",
            _ => "Telegram отклонил запрос.",
        };
        Err(ApiError {
            message: message.into(),
            retryable: code == 429 || code >= 500,
            auth: code == 401 || code == 404,
            retry_after: raw.parameters.and_then(|p| p.retry_after),
        })
    }

    pub async fn me(&self) -> Result<Bot, ApiError> {
        let bot: Bot = self.call("getMe", serde_json::json!({})).await?;
        if !bot.is_bot {
            return Err(ApiError::permanent("Токен не принадлежит боту."));
        }
        Ok(bot)
    }
    pub async fn react(&self, chat_id: i64, message_id: i64) -> Result<bool, ApiError> {
        self.call("setMessageReaction", serde_json::json!({"chat_id":chat_id,"message_id":message_id,"reaction":[{"type":"emoji","emoji":"⚡"}],"is_big":false})).await
    }
    pub async fn updates(&self, offset: i64) -> Result<Vec<Update>, ApiError> {
        self.call("getUpdates", serde_json::json!({ "offset":offset, "timeout":35, "limit":100, "allowed_updates":["message","channel_post"] })).await
    }
    pub async fn file(&self, id: &str) -> Result<RemoteFile, ApiError> {
        self.call("getFile", serde_json::json!({"file_id":id}))
            .await
    }
    pub async fn stream(&self, path: &str) -> Result<reqwest::Response, ApiError> {
        // Hosted API paths must never become local paths or arbitrary URLs.
        if path.starts_with('/') || path.contains("..") || path.contains([':', '\\', '?', '#']) {
            return Err(ApiError::permanent("Небезопасный путь файла Telegram."));
        }
        self.http
            .get(format!(
                "{}/file/bot{}/{path}",
                self.base,
                self.token.expose_secret()
            ))
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(ApiError::network)
    }
}

pub fn jobs(update: Update, bot_id: i64, settings: &Settings) -> (i64, Vec<Download>) {
    let update_id = update.update_id;
    let Some(message) = update.message.or(update.channel_post) else {
        return (update_id, vec![]);
    };
    if !settings.allowed_chat_ids.is_empty()
        && !settings.allowed_chat_ids.contains(&message.chat.id)
    {
        return (update_id, vec![]);
    }
    let media = if let Some(video) = message.video {
        Some((video, "video"))
    } else if let Some(photos) = message.photo {
        photos
            .into_iter()
            .max_by_key(|p| {
                (
                    p.file_size.unwrap_or(0),
                    p.width.unwrap_or(0) * p.height.unwrap_or(0),
                )
            })
            .map(|p| (p, "photo"))
    } else {
        message
            .document
            .filter(|d| {
                d.mime_type
                    .as_deref()
                    .is_some_and(|m| m.starts_with("video/"))
            })
            .map(|d| (d, "video"))
    };
    let Some((media, kind)) = media else {
        return (update_id, vec![]);
    };
    let ext = if kind == "photo" { "jpg" } else { "mp4" };
    let mut filename = sanitize_filename(
        &media
            .file_name
            .unwrap_or_else(|| format!("{kind}_{}_{}.{ext}", message.chat.id, message.message_id)),
    );
    let extension = std::path::Path::new(&filename)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if ![
        "jpg", "jpeg", "png", "webp", "mp4", "mov", "mkv", "webm", "avi", "m4v", "mpeg", "mpg",
        "3gp", "ts",
    ]
    .contains(&extension.as_str())
    {
        filename = format!("{filename}.{ext}");
    }
    let mut destination = std::path::PathBuf::from(&settings.destination);
    if settings.separate_folders {
        destination.push(if kind == "photo" { "Photos" } else { "Videos" });
    }
    let too_large = media.file_size.is_some_and(|n| n > HOSTED_LIMIT);
    (
        update_id,
        vec![Download {
            id: 0,
            bot_id,
            update_id,
            chat_id: message.chat.id,
            message_id: message.message_id,
            media_group_id: message.media_group_id,
            file_id: media.file_id,
            file_unique_id: media.file_unique_id,
            filename,
            media_type: kind.into(),
            total_bytes: media.file_size,
            downloaded_bytes: 0,
            speed: 0.0,
            status: if too_large { "failed" } else { "queued" }.into(),
            destination: destination.to_string_lossy().into(),
            final_path: None,
            temp_path: None,
            sha256: None,
            error: too_large.then(|| "Hosted Telegram Bot API: лимит скачивания 20 MB.".into()),
            attempts: 0,
            created_at: now(),
            started_at: None,
            completed_at: None,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };
    #[tokio::test]
    async fn validate_and_redact_auth_errors() {
        let server = MockServer::start().await;
        let mut api = Telegram::new(SecretString::from("123:SECRET_TOKEN".to_string())).unwrap();
        api.base = server.uri();
        Mock::given(method("POST"))
            .and(path("/bot123:SECRET_TOKEN/getMe"))
            .respond_with(ResponseTemplate::new(401).set_body_json(
                serde_json::json!({"ok":false,"error_code":401,"description":"SECRET_TOKEN"}),
            ))
            .mount(&server)
            .await;
        let error = api.me().await.unwrap_err();
        assert!(error.auth);
        assert!(!format!("{error:?}").contains("SECRET_TOKEN"));
    }
    #[test]
    fn albums_and_video_document_limit() {
        let update: Update = serde_json::from_value(serde_json::json!({"update_id":3,"message":{"message_id":4,"chat":{"id":5},"media_group_id":"album","document":{"file_id":"f","file_unique_id":"u","file_name":"../../bad.exe","mime_type":"video/mp4","file_size":30_000_000}}})).unwrap();
        let (_, items) = jobs(update, 1, &Settings::default());
        assert_eq!(items[0].status, "failed");
        assert!(items[0].filename.ends_with(".mp4"));
        assert_eq!(items[0].media_group_id.as_deref(), Some("album"));
    }

    #[tokio::test]
    async fn token_validation_and_rate_limit() {
        let server = MockServer::start().await;
        let api = Telegram::for_test(server.uri());
        Mock::given(path("/bot123:TEST_ONLY/getMe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok":true,"result":{"id":123,"username":"test_bot","is_bot":true}}))).mount(&server).await;
        assert_eq!(api.me().await.unwrap().id, 123);
        Mock::given(path("/bot123:TEST_ONLY/getUpdates"))
            .respond_with(ResponseTemplate::new(429).set_body_json(
                serde_json::json!({"ok":false,"error_code":429,"parameters":{"retry_after":17}}),
            ))
            .mount(&server)
            .await;
        let error = api.updates(1).await.unwrap_err();
        assert!(error.retryable);
        assert_eq!(error.retry_after, Some(17));
    }
}

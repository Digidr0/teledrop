use crate::{
    downloads,
    events::EventSink,
    models::*,
    security,
    storage::{MAX_PENDING, Storage},
    telegram::{self, Telegram},
};
use secrecy::SecretString;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    sync::Mutex as AsyncMutex,
    task::{JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;

struct Session {
    cancel: CancellationToken,
    handle: JoinHandle<()>,
}

pub struct Runtime {
    pub storage: Arc<Storage>,
    pub connection: Mutex<Connection>,
    pub sink: Arc<dyn EventSink>,
    pub jobs: Mutex<HashMap<i64, CancellationToken>>,
    session: AsyncMutex<Option<Session>>,
}

impl Runtime {
    pub fn new(storage: Arc<Storage>, sink: Arc<dyn EventSink>) -> Arc<Self> {
        Arc::new(Self {
            storage,
            sink,
            connection: Mutex::new(Connection::default()),
            jobs: Mutex::new(HashMap::new()),
            session: AsyncMutex::new(None),
        })
    }

    pub fn state(&self, state: &str, message: &str) {
        let mut connection = self.connection.lock().unwrap();
        let changed = connection.state != state || connection.message != message;
        connection.state = state.into();
        if ["connecting", "connected", "reconnecting"].contains(&state) {
            connection.running = true;
        }
        if ["stopped", "not_configured", "authentication_error"].contains(&state) {
            connection.running = false;
        }
        connection.message = message.into();
        if state == "connected" {
            connection.last_success = Some(now());
        }
        self.sink.connection(&connection);
        if changed {
            let _ = self.storage.log(
                if state.contains("error") {
                    "error"
                } else {
                    "info"
                },
                message,
            );
            tracing::info!(state, "connection state changed");
            self.sink.changed();
        }
    }

    async fn stop_locked(&self, session: &mut Option<Session>) {
        if let Some(session) = session.take() {
            session.cancel.cancel();
            let _ = session.handle.await;
        }
    }

    pub async fn disconnect(&self) {
        let mut session = self.session.lock().await;
        self.stop_locked(&mut session).await;
        self.state("stopped", "Получение остановлено. Очередь сохранена.");
    }

    pub async fn connect(self: &Arc<Self>) -> Result<(), String> {
        let mut session = self.session.lock().await;
        self.stop_locked(&mut session).await;
        let settings = self
            .storage
            .settings()
            .map_err(|_| "Не удалось прочитать настройки.")?;
        self.state("stopped", "Получение остановлено. Подготовка подключения.");
        crate::filesystem::validate_directory(&settings.destination)
            .await
            .map_err(|e| e.to_string())?;
        let token = security::load()
            .map_err(|_| "Credential Manager недоступен.")?
            .ok_or("Добавьте токен бота.")?;
        let api = Telegram::new(token).map_err(|_| "Не удалось создать HTTP-клиент.")?;
        self.state("connecting", "Проверяем бота и соединение с Telegram…");
        let cancel = CancellationToken::new();
        let runtime = self.clone();
        let child_cancel = cancel.clone();
        let handle = tokio::spawn(async move {
            runtime.authenticate(api, child_cancel).await;
        });
        *session = Some(Session { cancel, handle });
        Ok(())
    }

    async fn authenticate(self: Arc<Self>, api: Telegram, cancel: CancellationToken) {
        let mut attempt = 0_u32;
        loop {
            let result = tokio::select! {
                _ = cancel.cancelled() => return,
                result = api.me() => result,
            };
            match result {
                Ok(bot) => {
                    {
                        let mut connection = self.connection.lock().unwrap();
                        connection.bot_id = Some(bot.id);
                        connection.bot_username = bot.username;
                    }
                    self.session(api, bot.id, cancel).await;
                    return;
                }
                Err(error) => {
                    self.state(
                        if error.auth {
                            "authentication_error"
                        } else {
                            "network_error"
                        },
                        &error.message,
                    );
                    if !error.retryable {
                        let mut connection = self.connection.lock().unwrap();
                        connection.running = false;
                        self.sink.connection(&connection);
                        return;
                    }
                    let delay = downloads::retry_delay(
                        attempt,
                        error.retry_after,
                        rand::random_range(0..1000),
                    );
                    attempt = attempt.saturating_add(1);
                    tokio::select! {
                        _ = cancel.cancelled() => return,
                        _ = tokio::time::sleep(delay) => {},
                    }
                    self.state("reconnecting", "Повторяем подключение к Telegram…");
                }
            }
        }
    }

    pub async fn replace_token(self: &Arc<Self>, token: String) -> Result<(), String> {
        let token = SecretString::from(token.trim().to_owned());
        use secrecy::ExposeSecret;
        if token.expose_secret().len() < 10
            || !token.expose_secret().contains(':')
            || token
                .expose_secret()
                .chars()
                .any(|c| !(c.is_ascii_alphanumeric() || ":_-".contains(c)))
        {
            return Err("Неверный формат токена BotFather.".into());
        }
        let mut session = self.session.lock().await;
        let api = Telegram::new(token.clone()).map_err(|_| "Не удалось создать HTTP-клиент.")?;
        let bot = api.me().await.map_err(|e| e.message)?;
        self.stop_locked(&mut session).await;
        self.state(
            "stopped",
            "Получение остановлено. Сохраняем проверенный токен.",
        );
        security::save(token.expose_secret())
            .map_err(|_| "Не удалось сохранить токен в Credential Manager.")?;
        {
            let mut connection = self.connection.lock().unwrap();
            connection.bot_id = Some(bot.id);
            connection.bot_username = bot.username;
        }
        self.state("stopped", "Токен проверен и сохранён. Можно подключиться.");
        Ok(())
    }

    pub async fn remove_token(&self) -> Result<(), String> {
        let mut session = self.session.lock().await;
        self.stop_locked(&mut session).await;
        security::remove().map_err(|_| "Не удалось удалить токен.")?;
        *self.connection.lock().unwrap() = Connection::default();
        self.state(
            "not_configured",
            "Токен удалён. Добавьте токен бота в настройках.",
        );
        Ok(())
    }

    async fn session(self: Arc<Self>, api: Telegram, bot: i64, cancel: CancellationToken) {
        let poll_runtime = self.clone();
        let poll_cancel = cancel.clone();
        let poll_api = api.clone();
        let poller = tokio::spawn(async move {
            poll_runtime.poll(poll_api, bot, poll_cancel).await;
        });
        let mut workers = JoinSet::new();
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                result = workers.join_next(), if !workers.is_empty() => {
                    match result {
                        Some(Ok(id)) => { self.jobs.lock().unwrap().remove(&id); },
                        Some(Err(_)) => {
                            self.state("stopped","Worker завершился с ошибкой. Очередь будет восстановлена; подключитесь заново.");
                            cancel.cancel(); break;
                        },
                        None => {},
                    }
                },
                _ = tokio::time::sleep(Duration::from_millis(350)) => {
                    let Ok(settings) = self.storage.settings() else { self.state("stopped","Ошибка чтения базы. Получение остановлено."); cancel.cancel(); break; };
                    if workers.len() < settings.concurrency {
                        let mut active = self.jobs.lock().unwrap();
                        let tasks = match self.storage.queued(bot, settings.concurrency - workers.len()) {
                            Ok(tasks) => tasks,
                            Err(_) => { self.state("stopped", "Не удалось прочитать очередь SQLite."); cancel.cancel(); break; },
                        };
                        for mut task in tasks {
                            if active.contains_key(&task.id) { continue; }
                            task.status = "downloading".into(); task.started_at = Some(now()); task.error = None;
                            if self.storage.save(&task).is_err() { cancel.cancel(); break; }
                            let token = CancellationToken::new(); active.insert(task.id,token.clone());
                            self.sink.progress(&task); self.sink.changed();
                            let (api,db,sink,stop) = (api.clone(),self.storage.clone(),self.sink.clone(),cancel.clone());
                            workers.spawn(async move {
                                let id = task.id;
                                downloads::run(task,api,db,sink,token,stop,settings.notifications).await;
                                id
                            });
                        }
                    }
                }
            }
        }
        cancel.cancel();
        let _ = poller.await;
        while let Some(result) = workers.join_next().await {
            if let Ok(id) = result {
                self.jobs.lock().unwrap().remove(&id);
            }
        }
        // Reconcile any worker panic before a subsequent connection.
        let _ = downloads::recover(&self.storage).await;
        self.jobs.lock().unwrap().clear();
        {
            let mut connection = self.connection.lock().unwrap();
            connection.running = false;
            self.sink.connection(&connection);
        }
        self.sink.changed();
    }

    async fn poll(self: Arc<Self>, api: Telegram, bot: i64, cancel: CancellationToken) {
        let mut attempt = 0;
        loop {
            let Ok(settings) = self.storage.settings() else {
                self.state("stopped", "Не удалось прочитать настройки SQLite.");
                cancel.cancel();
                return;
            };
            let Ok(counts) = self.storage.counts() else {
                self.state("stopped", "Не удалось прочитать состояние очереди SQLite.");
                cancel.cancel();
                return;
            };
            if counts.queued + counts.downloading + 100 > MAX_PENDING as u64 {
                tokio::select! { _ = cancel.cancelled() => return, _ = tokio::time::sleep(Duration::from_secs(2)) => continue }
            }
            let Ok(offset) = self.storage.offset(bot) else {
                self.state("stopped", "Не удалось прочитать Telegram offset из SQLite.");
                cancel.cancel();
                return;
            };
            let result = tokio::select! {
                _ = cancel.cancelled() => return,
                value = api.updates(offset) => value,
            };
            match result {
                Ok(updates) => {
                    let starts = updates
                        .iter()
                        .filter(|u| u.update_id >= offset)
                        .filter_map(|u| u.message.as_ref().or(u.channel_post.as_ref()))
                        .filter(|m| {
                            (settings.allowed_chat_ids.is_empty()
                                || settings.allowed_chat_ids.contains(&m.chat.id))
                                && m.text
                                    .as_deref()
                                    .and_then(|t| t.split_whitespace().next())
                                    .is_some_and(|command| {
                                        let connection = self.connection.lock().unwrap();
                                        command == "/start"
                                            || connection.bot_username.as_ref().is_some_and(
                                                |name| {
                                                    command.eq_ignore_ascii_case(&format!(
                                                        "/start@{name}"
                                                    ))
                                                },
                                            )
                                    })
                        })
                        .count();
                    let jobs: Vec<_> = updates
                        .into_iter()
                        .map(|u| telegram::jobs(u, bot, &settings))
                        .collect();
                    if self
                        .storage
                        .ingest(bot, &jobs, settings.skip_duplicates)
                        .is_err()
                    {
                        self.state(
                            "stopped",
                            "Ошибка записи SQLite. Обновления Telegram не подтверждены.",
                        );
                        cancel.cancel();
                        return;
                    }
                    attempt = 0;
                    for _ in 0..starts {
                        self.sink.started();
                        let _ = self.storage.log(
                            "info",
                            "Получена команда /start; отправлено системное уведомление.",
                        );
                    }
                    self.state("connected", "Telegram подключён. Ожидаем фото и видео.");
                    if !jobs.is_empty() {
                        self.sink.changed();
                    }
                }
                Err(error) => {
                    self.state(
                        if error.auth {
                            "authentication_error"
                        } else if error.retryable {
                            "reconnecting"
                        } else {
                            "network_error"
                        },
                        &error.message,
                    );
                    if !error.retryable {
                        cancel.cancel();
                        return;
                    }
                    let delay = downloads::retry_delay(
                        attempt,
                        error.retry_after,
                        rand::random_range(0..1000),
                    );
                    attempt = attempt.saturating_add(1);
                    tokio::select! { _ = cancel.cancelled() => return, _ = tokio::time::sleep(delay) => {} }
                }
            }
        }
    }

    pub fn cancel_download(&self, id: i64) -> Result<(), String> {
        let active = self.jobs.lock().unwrap();
        let mut task = self.storage.get(id).map_err(|_| "Загрузка не найдена.")?;
        if let Some(token) = active.get(&id) {
            token.cancel();
        } else if task.status == "queued" {
            task.status = "cancelled".into();
            self.storage
                .save(&task)
                .map_err(|_| "Ошибка записи SQLite.")?;
            self.sink.changed();
        } else {
            return Err("Эту загрузку уже нельзя отменить.".into());
        }
        Ok(())
    }

    pub fn retry(&self, id: i64) -> Result<(), String> {
        let active = self.jobs.lock().unwrap();
        if active.contains_key(&id) {
            return Err("Загрузка ещё активна.".into());
        }
        let mut task = self.storage.get(id).map_err(|_| "Загрузка не найдена.")?;
        if !["failed", "cancelled"].contains(&task.status.as_str()) {
            return Err("Повтор доступен для неудачных и отменённых загрузок.".into());
        }
        task.status = "queued".into();
        task.error = None;
        task.downloaded_bytes = 0;
        task.final_path = None;
        task.sha256 = None;
        task.completed_at = None;
        self.storage.requeue(&task).map_err(
            |_| "Не удалось повторить загрузку. Проверьте состояние и свободное место в очереди.",
        )?;
        self.sink.changed();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    #[tokio::test]
    async fn connection_transitions_and_queue_controls() {
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let sink = Arc::new(TestSink::default());
        let runtime = Runtime::new(db.clone(), sink.clone());
        runtime.state("connecting", "test");
        runtime.state("connected", "test");
        runtime.state("reconnecting", "network");
        runtime.state("authentication_error", "revoked");
        runtime.disconnect().await;
        assert_eq!(
            *sink.connections.lock().unwrap(),
            [
                "connecting",
                "connected",
                "reconnecting",
                "authentication_error",
                "stopped"
            ]
        );
        assert!(!runtime.connection.lock().unwrap().running);
        let task = job(dir.path(), 1, "u");
        db.ingest(123, &[(1, vec![task])], false).unwrap();
        let id = db.queued(123, 1).unwrap()[0].id;
        runtime.cancel_download(id).unwrap();
        assert_eq!(db.get(id).unwrap().status, "cancelled");
        runtime.retry(id).unwrap();
        assert_eq!(db.get(id).unwrap().status, "queued");
        assert!(runtime.retry(id).is_err());
    }

    #[tokio::test]
    async fn polling_commits_jobs_before_next_offset_request() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_partial_json, path},
        };
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        db.save_settings(&Settings {
            destination: dir.path().to_string_lossy().into(),
            ..Default::default()
        })
        .unwrap();
        let server = MockServer::start().await;
        Mock::given(path("/bot123:TEST_ONLY/getUpdates"))
            .and(body_partial_json(serde_json::json!({"offset":0})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok":true,"result":[{"update_id":10,"message":{"message_id":11,"chat":{"id":12},"photo":[{"file_id":"f","file_unique_id":"u","file_size":3,"width":100,"height":100}]}}]})))
            .mount(&server).await;
        Mock::given(path("/bot123:TEST_ONLY/getUpdates"))
            .and(body_partial_json(serde_json::json!({"offset":11})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"ok":true,"result":[]}))
                    .set_delay(Duration::from_secs(30)),
            )
            .mount(&server)
            .await;
        let runtime = Runtime::new(db.clone(), Arc::new(TestSink::default()));
        let cancel = CancellationToken::new();
        let child_cancel = cancel.clone();
        let worker = tokio::spawn(async move {
            runtime
                .poll(Telegram::for_test(server.uri()), 123, child_cancel)
                .await;
            server
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while db.offset(123).unwrap() != 11 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(db.counts().unwrap().queued, 1);
        // Allow the next request to enter the mock, proving acknowledgement uses
        // the offset whose job has already been committed.
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
        let server = worker.await.unwrap();
        let requests = server.received_requests().await.unwrap();
        assert!(requests.iter().any(|r| {
            serde_json::from_slice::<serde_json::Value>(&r.body).unwrap()["offset"] == 11
        }));
        assert_eq!(db.queued(123, 1).unwrap()[0].update_id, 10);
    }
}

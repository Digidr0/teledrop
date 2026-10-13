use crate::{
    events::EventSink,
    filesystem,
    models::*,
    storage::Storage,
    telegram::{ApiError, HOSTED_LIMIT, Telegram},
};
use futures_util::StreamExt;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

pub fn retry_delay(attempt: u32, retry_after: Option<u64>, jitter: u64) -> Duration {
    let seconds = retry_after.unwrap_or_else(|| (1_u64 << attempt.min(6)).min(60));
    Duration::from_millis(seconds.saturating_mul(1000).saturating_add(jitter.min(999)))
}

enum TransferError {
    Api(ApiError),
    Cancelled(bool),
}
impl From<ApiError> for TransferError {
    fn from(error: ApiError) -> Self {
        Self::Api(error)
    }
}

fn check_cancellation(
    user: &CancellationToken,
    shutdown: &CancellationToken,
) -> Result<(), TransferError> {
    if user.is_cancelled() {
        return Err(TransferError::Cancelled(true));
    }
    if shutdown.is_cancelled() {
        return Err(TransferError::Cancelled(false));
    }
    Ok(())
}

async fn network<T>(
    future: impl std::future::Future<Output = Result<T, ApiError>>,
    user: &CancellationToken,
    shutdown: &CancellationToken,
) -> Result<T, TransferError> {
    tokio::select! {
        biased;
        _ = user.cancelled() => Err(TransferError::Cancelled(true)),
        _ = shutdown.cancelled() => Err(TransferError::Cancelled(false)),
        result = future => result.map_err(TransferError::Api),
    }
}

async fn clean_temp(task: &mut Download) {
    if let Some(path) = &task.temp_path {
        // Only remove this task's known .part, never scan arbitrary destination files.
        let path = Path::new(path);
        if path.parent() == Some(Path::new(&task.destination))
            && path.file_name().is_some_and(|s| {
                s.to_string_lossy().starts_with(".teledrop-")
                    && s.to_string_lossy().ends_with(".part")
            })
        {
            match tokio::fs::remove_file(path).await {
                Ok(()) => task.temp_path = None,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => task.temp_path = None,
                // Retain the persisted path so startup can retry cleanup.
                Err(_) => {}
            }
        }
    }
}

async fn published(task: &Download) -> bool {
    // A still-existing temporary means the no-clobber move never succeeded.
    // An identical pre-existing destination must not be mistaken for our move.
    if let Some(temporary) = &task.temp_path
        && !matches!(tokio::fs::try_exists(temporary).await, Ok(false))
    {
        return false;
    }
    if let (Some(path), Some(hash)) = (&task.final_path, &task.sha256) {
        return filesystem::sha256(Path::new(path))
            .await
            .is_ok_and(|h| &h == hash);
    }
    false
}

pub async fn recover(storage: &Storage) -> anyhow::Result<()> {
    for mut task in storage.incomplete()? {
        let incomplete = ["downloading", "finalizing"].contains(&task.status.as_str());
        let recovered = task.status == "finalizing" && published(&task).await;
        clean_temp(&mut task).await;
        if recovered {
            task.status = "completed".into();
            task.completed_at = Some(now());
            task.downloaded_bytes = task.total_bytes.unwrap_or(task.downloaded_bytes);
            task.error = None;
        } else if incomplete {
            task.status = "queued".into();
            task.downloaded_bytes = 0;
            task.error = Some("Прерванная загрузка будет начата заново.".into());
            task.final_path = None;
            task.sha256 = None;
        }
        task.speed = 0.0;
        storage.save(&task)?;
    }
    Ok(())
}

pub async fn run(
    mut task: Download,
    api: Telegram,
    storage: Arc<Storage>,
    sink: Arc<dyn EventSink>,
    cancelled: CancellationToken,
    shutdown: CancellationToken,
    notifications: bool,
) {
    for attempt in 0..5 {
        task.attempts += 1;
        // Do not drop Tokio filesystem operations: a blocking open/write may
        // otherwise finish after cleanup and leave an untracked .part behind.
        let result = transfer(&mut task, &api, &storage, &*sink, &cancelled, &shutdown).await;
        match result {
            Ok(()) => {
                task.status = "completed".into();
                task.speed = 0.0;
                task.completed_at = Some(now());
                task.error = None;
                if storage.save(&task).is_err() {
                    task.status = "finalizing".into();
                    task.error = Some("Файл записан, но завершение не сохранено в SQLite. Перезапустите приложение для восстановления.".into());
                    // Keep persisted finalizing intent for startup reconciliation.
                    let _ = storage.log("error","Не удалось сохранить завершение загрузки. Перезапустите приложение для восстановления.");
                } else {
                    let _ = storage.log(
                        "info",
                        &format!(
                            "Файл #{} сохранён ({} байт).",
                            task.id, task.downloaded_bytes
                        ),
                    );
                    if notifications {
                        sink.completed(&task.filename);
                    }
                    // Telegram applies album reactions to the referenced item.
                    // Only mark a message after its own file is durably saved.
                    let reaction = tokio::select! {
                        _ = shutdown.cancelled() => None,
                        result = tokio::time::timeout(Duration::from_secs(15), api.react(task.chat_id, task.message_id)) => Some(result),
                    };
                    if !matches!(reaction, Some(Ok(Ok(true)))) {
                        let _ = storage.log("warning", &format!("Файл #{} сохранён, но реакция ⚡ не отправлена: проверьте сеть и разрешённые реакции чата.",task.id));
                    }
                }
                sink.progress(&task);
                sink.changed();
                return;
            }
            Err(TransferError::Cancelled(user)) => {
                interrupted(&mut task, &storage, &*sink, user).await;
                return;
            }
            Err(TransferError::Api(error)) => {
                clean_temp(&mut task).await;
                task.downloaded_bytes = 0;
                task.speed = 0.0;
                task.error = Some(error.message.clone());
                if !error.retryable || attempt == 4 {
                    task.status = "failed".into();
                    let _ = storage.save(&task);
                    let _ = storage.log(
                        "error",
                        &format!("Загрузка #{}: {}", task.id, error.message),
                    );
                    sink.progress(&task);
                    sink.changed();
                    return;
                }
                let _ = storage.save(&task);
                sink.progress(&task);
                let delay = retry_delay(attempt, error.retry_after, rand::random_range(0..1000));
                tokio::select! {
                    _ = shutdown.cancelled() => { interrupted(&mut task, &storage, &*sink, false).await; return; },
                    _ = cancelled.cancelled() => { interrupted(&mut task, &storage, &*sink, true).await; return; },
                    _ = tokio::time::sleep(delay) => {},
                }
            }
        }
    }
}

async fn interrupted(task: &mut Download, storage: &Storage, sink: &dyn EventSink, user: bool) {
    // Cancellation during finalization must reconcile the destination instead of
    // blindly redownloading a file already atomically exposed to the user.
    if task.status == "finalizing" && published(task).await {
        task.status = "completed".into();
        task.completed_at = Some(now());
        clean_temp(task).await;
        task.speed = 0.0;
        let _ = storage.save(task);
        sink.progress(task);
        sink.changed();
        return;
    }
    clean_temp(task).await;
    task.status = if user { "cancelled" } else { "queued" }.into();
    task.final_path = None;
    task.sha256 = None;
    task.downloaded_bytes = 0;
    task.speed = 0.0;
    let _ = storage.save(task);
    sink.progress(task);
    sink.changed();
}

fn disk_error(e: std::io::Error) -> ApiError {
    let message = match e.kind() {
        std::io::ErrorKind::PermissionDenied => "Нет разрешения на запись в целевую папку.",
        std::io::ErrorKind::StorageFull => "Недостаточно свободного места на диске.",
        _ => "Не удалось записать файл. Проверьте диск и доступность папки.",
    };
    ApiError::permanent(message)
}

async fn transfer(
    task: &mut Download,
    api: &Telegram,
    storage: &Storage,
    sink: &dyn EventSink,
    cancelled: &CancellationToken,
    shutdown: &CancellationToken,
) -> Result<(), TransferError> {
    check_cancellation(cancelled, shutdown)?;
    clean_temp(task).await;
    if task.temp_path.is_some() {
        return Err(ApiError::permanent(
            "Не удалось удалить временный файл. Закройте приложения, блокирующие его, и повторите загрузку.",
        ).into());
    }
    if task.total_bytes.is_some_and(|n| n > HOSTED_LIMIT) {
        return Err(ApiError::permanent("Hosted Telegram Bot API: лимит 20 MB.").into());
    }
    let remote = network(api.file(&task.file_id), cancelled, shutdown).await?;
    if remote.file_size.is_some_and(|n| n > HOSTED_LIMIT) {
        return Err(ApiError::permanent("Hosted Telegram Bot API: лимит 20 MB.").into());
    }
    let path = remote
        .file_path
        .ok_or_else(|| ApiError::permanent("Telegram не вернул путь файла."))?;
    let response = network(api.stream(&path), cancelled, shutdown).await?;
    task.total_bytes = remote
        .file_size
        .or(task.total_bytes)
        .or(response.content_length());
    let directory = PathBuf::from(&task.destination);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(disk_error)?;
    check_cancellation(cancelled, shutdown)?;
    let temporary = directory.join(format!(
        ".teledrop-{}-{}.part",
        task.id,
        uuid::Uuid::new_v4()
    ));
    task.temp_path = Some(temporary.to_string_lossy().into());
    storage
        .save(task)
        .map_err(|_| ApiError::permanent("Не удалось сохранить состояние загрузки."))?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(disk_error)?;
    let mut stream = response.bytes_stream();
    let mut last_event = Instant::now();
    let mut last_bytes = 0;
    task.downloaded_bytes = 0;
    loop {
        let chunk = network(
            async { stream.next().await.transpose().map_err(ApiError::network) },
            cancelled,
            shutdown,
        )
        .await?;
        let Some(chunk) = chunk else {
            break;
        };
        task.downloaded_bytes += chunk.len() as u64;
        if task.downloaded_bytes > HOSTED_LIMIT {
            return Err(ApiError::permanent("Превышен лимит 20 MB.").into());
        }
        file.write_all(&chunk).await.map_err(disk_error)?;
        if last_event.elapsed() >= Duration::from_millis(250) {
            task.speed =
                (task.downloaded_bytes - last_bytes) as f64 / last_event.elapsed().as_secs_f64();
            last_bytes = task.downloaded_bytes;
            last_event = Instant::now();
            sink.progress(task);
        }
    }
    if task.total_bytes.is_some_and(|n| n != task.downloaded_bytes) {
        return Err(ApiError {
            message: "Файл получен не полностью.".into(),
            retryable: true,
            auth: false,
            retry_after: None,
        }
        .into());
    }
    task.total_bytes = Some(task.downloaded_bytes);
    file.flush().await.map_err(disk_error)?;
    file.sync_all().await.map_err(disk_error)?;
    drop(file);
    task.sha256 = Some(
        filesystem::sha256(&temporary)
            .await
            .map_err(|_| ApiError::permanent("Не удалось проверить файл."))?,
    );
    check_cancellation(cancelled, shutdown)?;
    task.status = "finalizing".into();
    for index in 0..10_000 {
        let target = filesystem::collision_path(&directory, &task.filename, index);
        task.final_path = Some(target.to_string_lossy().into());
        // Persist intent before an atomic same-volume no-clobber Windows move.
        storage
            .save(task)
            .map_err(|_| ApiError::permanent("Не удалось сохранить финализацию."))?;
        match filesystem::publish_no_replace(&temporary, &target) {
            Ok(()) => {
                task.temp_path = None;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(disk_error(error).into()),
        }
    }
    Err(ApiError::permanent("Слишком много совпадающих имён файлов.").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backoff_is_bounded_and_respects_retry_after() {
        assert_eq!(retry_delay(0, None, 0), Duration::from_secs(1));
        assert_eq!(retry_delay(20, None, 0), Duration::from_secs(60));
        assert_eq!(retry_delay(1, Some(90), 400), Duration::from_millis(90400));
    }
    #[tokio::test]
    async fn no_clobber_finalization() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("a.part");
        let dest = dir.path().join("a.mp4");
        tokio::fs::write(&tmp, b"complete").await.unwrap();
        tokio::fs::write(&dest, b"existing").await.unwrap();
        assert!(filesystem::publish_no_replace(&tmp, &dest).is_err());
        assert_eq!(tokio::fs::read(dest).await.unwrap(), b"existing");
    }

    #[tokio::test]
    async fn streaming_download_collision_and_history() {
        use crate::test_support::*;
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let job = job(dir.path(), 1, "u");
        db.ingest(123, &[(1, vec![job])], false).unwrap();
        let mut task = db.queued(123, 1).unwrap().pop().unwrap();
        task.status = "downloading".into();
        db.save(&task).unwrap();
        tokio::fs::write(dir.path().join("clip.mp4"), b"existing")
            .await
            .unwrap();
        let server = MockServer::start().await;
        Mock::given(path("/bot123:TEST_ONLY/getFile")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok":true,"result":{"file_path":"videos/clip.mp4","file_size":8}}))).mount(&server).await;
        Mock::given(path("/file/bot123:TEST_ONLY/videos/clip.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"complete"))
            .mount(&server)
            .await;
        let id = task.id;
        run(
            task,
            Telegram::for_test(server.uri()),
            db.clone(),
            Arc::new(TestSink::default()),
            CancellationToken::new(),
            CancellationToken::new(),
            false,
        )
        .await;
        let result = db.get(id).unwrap();
        assert_eq!(result.status, "completed");
        assert_eq!(result.downloaded_bytes, 8);
        assert_eq!(
            tokio::fs::read(dir.path().join("clip.mp4")).await.unwrap(),
            b"existing"
        );
        assert_eq!(
            tokio::fs::read(dir.path().join("clip (1).mp4"))
                .await
                .unwrap(),
            b"complete"
        );
        assert!(result.temp_path.is_none());
    }

    #[tokio::test]
    async fn interrupted_tasks_restart_and_finalized_tasks_reconcile() {
        use crate::test_support::*;
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let job = job(dir.path(), 1, "u");
        db.ingest(123, &[(1, vec![job])], false).unwrap();
        let mut task = db.queued(123, 1).unwrap().pop().unwrap();
        let temporary = dir.path().join(".teledrop-test.part");
        tokio::fs::write(&temporary, b"partial").await.unwrap();
        task.temp_path = Some(temporary.to_string_lossy().into());
        task.status = "downloading".into();
        task.downloaded_bytes = 7;
        db.save(&task).unwrap();
        recover(&db).await.unwrap();
        assert_eq!(db.get(task.id).unwrap().status, "queued");
        assert_eq!(db.get(task.id).unwrap().downloaded_bytes, 0);
        assert!(!temporary.exists());
        let final_path = dir.path().join("clip.mp4");
        tokio::fs::write(&final_path, b"complete").await.unwrap();
        task.final_path = Some(final_path.to_string_lossy().into());
        task.sha256 = Some(filesystem::sha256(&final_path).await.unwrap());
        task.status = "finalizing".into();
        db.save(&task).unwrap();
        recover(&db).await.unwrap();
        assert_eq!(db.get(task.id).unwrap().status, "completed");
    }
    #[tokio::test]
    async fn cancellation_is_persistent_and_safe() {
        use crate::test_support::*;
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let job = job(dir.path(), 1, "u");
        db.ingest(123, &[(1, vec![job])], false).unwrap();
        let task = db.queued(123, 1).unwrap().pop().unwrap();
        let id = task.id;
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        run(
            task,
            Telegram::for_test("http://127.0.0.1:1".into()),
            db.clone(),
            Arc::new(TestSink::default()),
            cancelled,
            CancellationToken::new(),
            false,
        )
        .await;
        assert_eq!(db.get(id).unwrap().status, "cancelled");
        assert_eq!(db.get(id).unwrap().downloaded_bytes, 0);
    }

    #[tokio::test]
    async fn preexisting_identical_file_is_not_a_successful_move() {
        use crate::test_support::*;
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let sample = job(dir.path(), 1, "unique");
        db.ingest(123, &[(1, vec![sample])], false).unwrap();
        let mut task = db.queued(123, 1).unwrap().pop().unwrap();
        let temporary = dir.path().join(".teledrop-owned.part");
        let target = dir.path().join("clip.mp4");
        tokio::fs::write(&temporary, b"complete").await.unwrap();
        tokio::fs::write(&target, b"complete").await.unwrap();
        task.status = "finalizing".into();
        task.temp_path = Some(temporary.to_string_lossy().into());
        task.final_path = Some(target.to_string_lossy().into());
        task.sha256 = Some(filesystem::sha256(&target).await.unwrap());
        db.save(&task).unwrap();
        recover(&db).await.unwrap();
        assert_eq!(db.get(task.id).unwrap().status, "queued");
        assert_eq!(tokio::fs::read(target).await.unwrap(), b"complete");
    }

    #[tokio::test]
    async fn recovery_never_removes_an_unowned_file() {
        use crate::test_support::*;
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let sample = job(dir.path(), 1, "unique");
        db.ingest(123, &[(1, vec![sample])], false).unwrap();
        let mut task = db.queued(123, 1).unwrap().pop().unwrap();
        let user_file = dir.path().join("my-video.part");
        tokio::fs::write(&user_file, b"user content").await.unwrap();
        task.temp_path = Some(user_file.to_string_lossy().into());
        task.status = "downloading".into();
        db.save(&task).unwrap();
        recover(&db).await.unwrap();
        assert_eq!(tokio::fs::read(user_file).await.unwrap(), b"user content");
        assert!(db.get(task.id).unwrap().temp_path.is_some());
    }
}

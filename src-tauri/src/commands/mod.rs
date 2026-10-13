use crate::{filesystem, models::*, runtime::Runtime, security};
use std::sync::Arc;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

type Backend<'a> = State<'a, Arc<Runtime>>;

#[tauri::command]
pub async fn check_update() -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("TeleDrop update check")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| "Проверка обновлений недоступна.")?;
    let response = client
        .get("https://api.github.com/repos/Digidr0/teledrop/releases/latest")
        .send()
        .await
        .map_err(|_| "GitHub недоступен.")?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    let release: serde_json::Value = response
        .error_for_status()
        .map_err(|_| "GitHub отклонил проверку обновлений.")?
        .json()
        .await
        .map_err(|_| "Некорректный ответ GitHub.")?;
    if release["draft"].as_bool() != Some(false) || release["prerelease"].as_bool() != Some(false) {
        return Ok(None);
    }
    let tag = release["tag_name"].as_str().ok_or("Нет версии релиза.")?;
    let version = semver::Version::parse(tag.trim_start_matches('v'))
        .map_err(|_| "Некорректная версия релиза.")?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|_| "Некорректная версия приложения.")?;
    Ok((version > current && version.pre.is_empty()).then(|| version.to_string()))
}

#[tauri::command]
pub fn open_update() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("https://github.com/Digidr0/teledrop/releases/latest")
        .spawn()
        .map(|_| ())
        .map_err(|_| "Не удалось открыть страницу релиза.".into())
}

#[tauri::command]
pub async fn check_telegram_api() -> Result<u64, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| "Не удалось создать HTTP-клиент.")?;
    let started = std::time::Instant::now();
    let response = client
        .get("https://api.telegram.org")
        .send()
        .await
        .map_err(|_| "Нет ответа api.telegram.org: возможны блокировка, сбой DNS, VPN или сети.")?;
    if response.status().is_server_error() {
        return Err("Сервер Telegram API отвечает, но временно недоступен.".into());
    }
    Ok(started.elapsed().as_millis() as u64)
}

fn db_error(_: impl std::fmt::Display) -> String {
    "Не удалось прочитать или записать локальную базу данных.".into()
}

#[tauri::command]
pub fn snapshot(runtime: Backend<'_>) -> Result<Snapshot, String> {
    Ok(Snapshot {
        settings: runtime.storage.settings().map_err(db_error)?,
        connection: runtime.connection.lock().unwrap().clone(),
        counts: runtime.storage.counts().map_err(db_error)?,
        downloads: runtime.storage.visible_downloads().map_err(db_error)?,
        activity: runtime.storage.activity().map_err(db_error)?,
        token_configured: security::load()
            .map_err(|_| "Credential Manager недоступен.")?
            .is_some(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

#[tauri::command]
pub fn history(runtime: Backend<'_>, query: HistoryQuery) -> Result<HistoryPage, String> {
    if query.search.len() > 500 {
        return Err("Поисковый запрос слишком длинный.".into());
    }
    runtime.storage.history(&query).map_err(db_error)
}

#[tauri::command]
pub async fn connect(runtime: Backend<'_>) -> Result<(), String> {
    runtime.inner().connect().await
}

#[tauri::command]
pub async fn disconnect(runtime: Backend<'_>) -> Result<(), String> {
    runtime.disconnect().await;
    Ok(())
}

#[tauri::command]
pub async fn validate_and_save_token(runtime: Backend<'_>, token: String) -> Result<(), String> {
    runtime.inner().replace_token(token).await
}

#[tauri::command]
pub async fn remove_token(runtime: Backend<'_>) -> Result<(), String> {
    runtime.remove_token().await
}

pub fn apply_theme(app: &tauri::AppHandle, theme: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_theme(match theme {
                "light" => Some(tauri::Theme::Light),
                "dark" => Some(tauri::Theme::Dark),
                _ => None,
            })
            .map_err(|_| "Не удалось изменить тему окна.")?;
    }
    Ok(())
}

#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    runtime: Backend<'_>,
    settings: Settings,
) -> Result<(), String> {
    settings.validate()?;
    let old = runtime.storage.settings().map_err(db_error)?;
    if settings.destination != old.destination && !settings.destination.is_empty() {
        filesystem::validate_directory(&settings.destination)
            .await
            .map_err(|e| e.to_string())?;
    }
    runtime.storage.save_settings(&settings).map_err(db_error)?;
    apply_theme(&app, &settings.theme)?;
    runtime.sink.changed();
    Ok(())
}

#[tauri::command]
pub async fn browse_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("TeleDrop — папка для фото и видео")
            .blocking_pick_folder()
            .map(|path| {
                path.into_path()
                    .map(|p| p.to_string_lossy().to_string())
                    .map_err(|_| "Не удалось прочитать путь папки.".into())
            })
            .transpose()
    })
    .await
    .map_err(|_| "Не удалось открыть диалог выбора папки.".to_string())?
}

#[tauri::command]
pub fn open_destination(runtime: Backend<'_>) -> Result<(), String> {
    let settings = runtime.storage.settings().map_err(db_error)?;
    if settings.destination.is_empty() {
        return Err("Сначала выберите папку.".into());
    }
    filesystem::open(std::path::Path::new(&settings.destination), false)
        .map_err(|_| "Не удалось открыть папку.".into())
}

#[tauri::command]
pub fn open_download(runtime: Backend<'_>, id: i64, reveal: bool) -> Result<(), String> {
    let task = runtime.storage.get(id).map_err(db_error)?;
    if task.status != "completed" {
        return Err("Файл ещё не сохранён.".into());
    }
    let path = task.final_path.ok_or("Нет пути к файлу.")?;
    filesystem::open(std::path::Path::new(&path), reveal)
        .map_err(|_| "Не удалось открыть файл. Возможно, он перемещён или удалён.".into())
}

#[tauri::command]
pub fn cancel_download(runtime: Backend<'_>, id: i64) -> Result<(), String> {
    runtime.cancel_download(id)
}

#[tauri::command]
pub fn retry_download(runtime: Backend<'_>, id: i64) -> Result<(), String> {
    runtime.retry(id)
}

#[tauri::command]
pub fn open_logs(app: tauri::AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .app_log_dir()
        .map_err(|_| "Папка логов недоступна.")?;
    filesystem::open(&path, false).map_err(|_| "Не удалось открыть папку логов.".into())
}

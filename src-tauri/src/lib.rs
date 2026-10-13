mod commands;
mod downloads;
mod events;
mod filesystem;
mod models;
mod runtime;
mod security;
mod storage;
mod telegram;
#[cfg(test)]
mod test_support;

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use tauri::Manager;

pub fn run() {
    let exit_state = Arc::new(AtomicU8::new(0));
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::check_update,
            commands::open_update,
            commands::check_telegram_api,
            commands::snapshot,
            commands::history,
            commands::connect,
            commands::disconnect,
            commands::validate_and_save_token,
            commands::remove_token,
            commands::save_settings,
            commands::browse_folder,
            commands::open_destination,
            commands::open_download,
            commands::cancel_download,
            commands::retry_download,
            commands::open_logs,
        ])
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let logs = app.path().app_log_dir()?;
            std::fs::create_dir_all(&data)?;
            std::fs::create_dir_all(&logs)?;
            let writer = tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("teledrop")
                .filename_suffix("log")
                .max_log_files(7)
                .build(&logs)?;
            // Fixed filter: never enable HTTP wire debugging via environment variables.
            tracing_subscriber::fmt()
                .with_ansi(false)
                .with_env_filter("teledrop=info,app_lib=info")
                .with_writer(writer)
                .try_init()
                .ok();
            let db = Arc::new(storage::Storage::open(&data.join("teledrop.sqlite"))?);
            tauri::async_runtime::block_on(downloads::recover(&db))?;
            let settings = db.settings()?;
            commands::apply_theme(app.handle(), &settings.theme).map_err(anyhow::Error::msg)?;
            let sink = Arc::new(events::DesktopEvents(app.handle().clone()));
            let runtime = runtime::Runtime::new(db, sink);
            if security::load()?.is_some() {
                runtime.state("stopped", "Бот настроен. Нажмите «Подключиться».");
            }
            app.manage(runtime.clone());
            tauri::async_runtime::spawn(async move {
                runtime.sink.changed();
                if settings.auto_connect {
                    let _ = runtime.connect().await;
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Не удалось запустить TeleDrop");
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if exit_state.load(Ordering::SeqCst) != 2 {
                api.prevent_exit();
            }
            if exit_state
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let app = app.clone();
                let runtime = app.state::<Arc<runtime::Runtime>>().inner().clone();
                let exit_state = exit_state.clone();
                tauri::async_runtime::spawn(async move {
                    runtime.disconnect().await;
                    exit_state.store(2, Ordering::SeqCst);
                    app.exit(0);
                });
            }
        }
    });
}

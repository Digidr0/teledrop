use crate::models::{Connection, Download};
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

pub trait EventSink: Send + Sync {
    fn connection(&self, state: &Connection);
    fn progress(&self, task: &Download);
    fn changed(&self);
    fn completed(&self, filename: &str);
    fn started(&self) {}
}

pub struct DesktopEvents(pub tauri::AppHandle);
impl EventSink for DesktopEvents {
    fn started(&self) {
        let _ = self
            .0
            .notification()
            .builder()
            .title("TeleDrop — /start")
            .body("Бот получил команду /start. Можно отправлять фото и видео.")
            .show();
    }
    fn connection(&self, state: &Connection) {
        let _ = self.0.emit("connection", state);
    }
    fn progress(&self, task: &Download) {
        let _ = self.0.emit("download-progress", task);
    }
    fn changed(&self) {
        let _ = self.0.emit("data-changed", ());
    }
    fn completed(&self, filename: &str) {
        let _ = self
            .0
            .notification()
            .builder()
            .title("TeleDrop — файл сохранён")
            .body(filename)
            .show();
    }
}

use crate::{events::EventSink, models::*, storage::Storage, telegram};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub struct TestSink {
    pub connections: Mutex<Vec<String>>,
    pub downloads: Mutex<Vec<Download>>,
}
impl EventSink for TestSink {
    fn connection(&self, value: &Connection) {
        self.connections.lock().unwrap().push(value.state.clone());
    }
    fn progress(&self, value: &Download) {
        self.downloads.lock().unwrap().push(value.clone());
    }
    fn changed(&self) {}
    fn completed(&self, _: &str) {}
}
pub fn database(dir: &Path) -> Arc<Storage> {
    Arc::new(Storage::open(&dir.join("test.sqlite")).unwrap())
}
pub fn job(dir: &Path, update_id: i64, unique: &str) -> Download {
    let update = serde_json::from_value(serde_json::json!({"update_id":update_id,"message":{"message_id":update_id,"chat":{"id":1},"video":{"file_id":"file","file_unique_id":unique,"file_name":"clip.mp4","file_size":8}}})).unwrap();
    let settings = Settings {
        destination: dir.to_string_lossy().into(),
        ..Default::default()
    };
    telegram::jobs(update, 123, &settings).1.pop().unwrap()
}

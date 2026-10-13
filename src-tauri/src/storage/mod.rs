use crate::models::*;
use rusqlite::{Connection as Sqlite, params};
use std::{path::Path, sync::Mutex};

pub const MAX_PENDING: usize = 10_000;
pub struct Storage {
    db: Mutex<Sqlite>,
}

impl Storage {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let mut db = Sqlite::open(path)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            anyhow::bail!("База создана более новой версией TeleDrop.");
        }
        if version == 0 {
            let transaction = db.transaction()?;
            transaction.execute_batch(include_str!("migrations/001.sql"))?;
            transaction.commit()?;
        }
        Ok(Self { db: Mutex::new(db) })
    }

    pub fn settings(&self) -> anyhow::Result<Settings> {
        use rusqlite::OptionalExtension;
        let db = self.db.lock().unwrap();
        let raw: Option<String> = db
            .query_row("SELECT value FROM settings WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let value: Settings = raw
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        value.validate().map_err(anyhow::Error::msg)?;
        Ok(value)
    }

    pub fn save_settings(&self, value: &Settings) -> anyhow::Result<()> {
        value.validate().map_err(anyhow::Error::msg)?;
        self.db.lock().unwrap().execute("INSERT INTO settings VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value", [serde_json::to_string(value)?])?;
        Ok(())
    }

    pub fn offset(&self, bot: i64) -> anyhow::Result<i64> {
        use rusqlite::OptionalExtension;
        Ok(self
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT next_offset FROM offsets WHERE bot_id=?1",
                [bot],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// Offset and jobs are committed together before Telegram acknowledgement.
    pub fn ingest(
        &self,
        bot: i64,
        updates: &[(i64, Vec<Download>)],
        skip_duplicates: bool,
    ) -> anyhow::Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let pending: usize = tx.query_row(
            "SELECT count(*) FROM downloads WHERE status IN ('queued','downloading','finalizing')",
            [],
            |r| r.get(0),
        )?;
        if pending + updates.iter().map(|(_, d)| d.len()).sum::<usize>() > MAX_PENDING {
            anyhow::bail!("Очередь заполнена.");
        }
        for (update_id, jobs) in updates {
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO updates VALUES (?1, ?2, ?3)",
                params![bot, update_id, now()],
            )?;
            if inserted > 0 {
                for job in jobs {
                    let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM downloads WHERE bot_id=?1 AND file_unique_id=?2 AND status IN ('queued','downloading','finalizing','completed'))", params![bot, job.file_unique_id], |r| r.get(0))?;
                    if skip_duplicates && exists {
                        continue;
                    }
                    tx.execute("INSERT OR IGNORE INTO downloads(bot_id,update_id,file_unique_id,status,media_type,filename,created_at,data) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![bot, update_id, job.file_unique_id, job.status, job.media_type, job.filename, job.created_at, serde_json::to_string(job)?])?;
                }
            }
        }
        if let Some((last, _)) = updates.iter().max_by_key(|(id, _)| id) {
            tx.execute("INSERT INTO offsets VALUES (?1, ?2) ON CONFLICT(bot_id) DO UPDATE SET next_offset=max(offsets.next_offset,excluded.next_offset)", params![bot, last + 1])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn decode(id: i64, raw: String) -> anyhow::Result<Download> {
        let mut d: Download = serde_json::from_str(&raw)?;
        d.id = id;
        Ok(d)
    }

    pub fn get(&self, id: i64) -> anyhow::Result<Download> {
        let (id, data) = self.db.lock().unwrap().query_row(
            "SELECT id,data FROM downloads WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Self::decode(id, data)
    }

    pub fn save(&self, d: &Download) -> anyhow::Result<()> {
        self.db.lock().unwrap().execute(
            "UPDATE downloads SET status=?1,data=?2 WHERE id=?3",
            params![d.status, serde_json::to_string(d)?, d.id],
        )?;
        Ok(())
    }

    pub fn requeue(&self, task: &Download) -> anyhow::Result<()> {
        // The same DB lock serializes this capacity check with ingestion.
        let db = self.db.lock().unwrap();
        let pending: usize = db.query_row(
            "SELECT count(*) FROM downloads WHERE status IN ('queued','downloading','finalizing')",
            [],
            |r| r.get(0),
        )?;
        if pending >= MAX_PENDING {
            anyhow::bail!("Очередь заполнена.");
        }
        let updated = db.execute("UPDATE downloads SET status='queued',data=?1 WHERE id=?2 AND status IN ('failed','cancelled')", params![serde_json::to_string(task)?,task.id])?;
        if updated != 1 {
            anyhow::bail!("Задание уже изменило состояние.");
        }
        Ok(())
    }

    pub fn queued(&self, bot: i64, limit: usize) -> anyhow::Result<Vec<Download>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare("SELECT id,data FROM downloads WHERE bot_id=?1 AND status='queued' ORDER BY id LIMIT ?2")?;
        let rows = stmt.query_map(params![bot, limit], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.map(|row| {
            let (id, raw) = row?;
            Self::decode(id, raw)
        })
        .collect()
    }

    pub fn incomplete(&self) -> anyhow::Result<Vec<Download>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id,data FROM downloads WHERE status IN ('downloading','finalizing') OR json_extract(data,'$.tempPath') IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.map(|row| {
            let (id, raw) = row?;
            Self::decode(id, raw)
        })
        .collect()
    }

    pub fn visible_downloads(&self) -> anyhow::Result<Vec<Download>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare("SELECT id,data FROM downloads WHERE id IN (SELECT id FROM (SELECT id FROM downloads WHERE status IN ('queued','downloading','finalizing') ORDER BY CASE WHEN status='queued' THEN 1 ELSE 0 END,id LIMIT 100) UNION SELECT id FROM (SELECT id FROM downloads ORDER BY id DESC LIMIT 50)) ORDER BY CASE status WHEN 'downloading' THEN 0 WHEN 'finalizing' THEN 0 WHEN 'queued' THEN 1 ELSE 2 END,id DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.map(|row| {
            let (id, raw) = row?;
            Self::decode(id, raw)
        })
        .collect()
    }

    pub fn history(&self, q: &HistoryQuery) -> anyhow::Result<HistoryPage> {
        let db = self.db.lock().unwrap();
        let search = format!(
            "%{}%",
            q.search
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let conditions = "WHERE filename LIKE ?1 ESCAPE '\\' AND (?2='' OR status=?2) AND (?3='' OR media_type=?3)";
        let total = db.query_row(
            &format!("SELECT count(*) FROM downloads {conditions}"),
            params![search, q.status, q.media_type],
            |r| r.get(0),
        )?;
        let mut stmt = db.prepare(&format!(
            "SELECT id,data FROM downloads {conditions} ORDER BY id DESC LIMIT 50 OFFSET ?4"
        ))?;
        let rows = stmt.query_map(
            params![
                search,
                q.status,
                q.media_type,
                i64::from(q.page.min(1_000_000)) * 50
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let items = rows
            .map(|row| {
                let (id, raw) = row?;
                Self::decode(id, raw)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(HistoryPage { items, total })
    }

    pub fn counts(&self) -> anyhow::Result<Counts> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare("SELECT status,count(*) FROM downloads GROUP BY status")?;
        let mut result = Counts::default();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))? {
            let (state, n) = row?;
            match state.as_str() {
                "queued" => result.queued = n,
                "downloading" | "finalizing" => result.downloading += n,
                "completed" => result.completed = n,
                "failed" => result.failed = n,
                "cancelled" => result.cancelled = n,
                _ => {}
            }
        }
        Ok(result)
    }

    pub fn log(&self, level: &str, message: &str) -> anyhow::Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO activity(time,level,message) VALUES (?1,?2,?3)",
            params![now(), level, message],
        )?;
        tx.execute(
            "DELETE FROM activity WHERE id <= (SELECT max(id)-2000 FROM activity)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn activity(&self) -> anyhow::Result<Vec<Activity>> {
        let db = self.db.lock().unwrap();
        let mut stmt =
            db.prepare("SELECT id,time,level,message FROM activity ORDER BY id DESC LIMIT 100")?;
        Ok(stmt
            .query_map([], |r| {
                Ok(Activity {
                    id: r.get(0)?,
                    time: r.get(1)?,
                    level: r.get(2)?,
                    message: r.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrations_settings_and_durable_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let db = Storage::open(&path).unwrap();
        let s = Settings {
            theme: "dark".into(),
            ..Default::default()
        };
        db.save_settings(&s).unwrap();
        db.ingest(123, &[(7, vec![])], true).unwrap();
        db.ingest(123, &[(7, vec![])], true).unwrap();
        drop(db);
        let db = Storage::open(&path).unwrap();
        assert_eq!(db.settings().unwrap(), s);
        assert_eq!(db.offset(123).unwrap(), 8);
        assert_eq!(db.offset(456).unwrap(), 0);
        assert_eq!(db.history(&HistoryQuery::default()).unwrap().total, 0);
    }

    #[test]
    fn update_and_media_deduplication_and_queue_order() {
        use crate::test_support::*;
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let job = job(dir.path(), 7, "unique");
        db.ingest(123, &[(7, vec![job.clone()])], true).unwrap();
        db.ingest(123, &[(7, vec![job.clone()])], false).unwrap();
        let mut duplicate = job.clone();
        duplicate.update_id = 8;
        db.ingest(123, &[(8, vec![duplicate.clone()])], true)
            .unwrap();
        assert_eq!(db.history(&HistoryQuery::default()).unwrap().total, 1);
        duplicate.update_id = 9;
        db.ingest(123, &[(9, vec![duplicate])], false).unwrap();
        let items = db.queued(123, 1).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].update_id, 7);
        assert_eq!(db.counts().unwrap().queued, 2);
        assert_eq!(db.offset(123).unwrap(), 10);
        assert_eq!(
            db.history(&HistoryQuery {
                search: "not found".into(),
                ..Default::default()
            })
            .unwrap()
            .total,
            0
        );
        assert_eq!(db.visible_downloads().unwrap().len(), 2);
    }
    #[test]
    fn old_settings_receive_new_defaults() {
        let old: Settings = serde_json::from_str(r#"{"theme":"dark","concurrency":3}"#).unwrap();
        assert!(old.skip_duplicates);
        assert_eq!(old.theme, "dark");
        assert_eq!(old.concurrency, 3);
        assert!(old.allowed_chat_ids.is_empty());
        assert!(
            Settings {
                concurrency: 0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn full_queue_does_not_advance_offset() {
        use crate::test_support::*;
        let dir = tempfile::tempdir().unwrap();
        let db = database(dir.path());
        let sample = job(dir.path(), 1, "unique");
        assert!(
            db.ingest(123, &[(1, vec![sample; MAX_PENDING + 1])], false)
                .is_err()
        );
        assert_eq!(db.offset(123).unwrap(), 0);
        assert_eq!(db.counts().unwrap().queued, 0);
    }
}

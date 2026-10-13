CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK(id = 1), value TEXT NOT NULL);
CREATE TABLE offsets (bot_id INTEGER PRIMARY KEY, next_offset INTEGER NOT NULL);
CREATE TABLE updates (bot_id INTEGER NOT NULL, update_id INTEGER NOT NULL, received_at INTEGER NOT NULL, PRIMARY KEY(bot_id, update_id));
CREATE TABLE downloads (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 bot_id INTEGER NOT NULL, update_id INTEGER NOT NULL,
 file_unique_id TEXT NOT NULL, status TEXT NOT NULL,
 media_type TEXT NOT NULL, filename TEXT NOT NULL,
 created_at INTEGER NOT NULL, data TEXT NOT NULL,
 UNIQUE(bot_id, update_id, file_unique_id)
);
CREATE INDEX downloads_state ON downloads(bot_id, status, id);
CREATE INDEX downloads_duplicate ON downloads(bot_id, file_unique_id, status);
CREATE INDEX downloads_history ON downloads(created_at DESC, id DESC);
CREATE TABLE activity (id INTEGER PRIMARY KEY AUTOINCREMENT, time INTEGER NOT NULL, level TEXT NOT NULL, message TEXT NOT NULL);
PRAGMA user_version = 1;

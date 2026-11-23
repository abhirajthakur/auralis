use auralis_core::config::Settings;
use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct SettingsStore { conn: Mutex<Connection> }

impl SettingsStore {
    pub fn new() -> SqlResult<Self> {
        let path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Auralis")
            .join("auralis.db");
        if let Some(p) = path.parent() { std::fs::create_dir_all(p).ok(); }
        let conn = Connection::open(&path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn load(&self) -> SqlResult<Settings> {
        let conn = self.conn.lock().unwrap();
        let row: Result<(String,), _> = conn.query_row(
            "SELECT value FROM settings WHERE key = 'root'",
            [],
            |r| Ok((r.get(0)?,)),
        );
        match row {
            Ok((json,)) => serde_json::from_str(&json).map_err(|_| rusqlite::Error::InvalidQuery),
            Err(_) => Ok(Settings::default()),
        }
    }

    pub fn save(&self, settings: &Settings) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(settings).map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('root', ?)",
            [&json],
        )?;
        Ok(())
    }
}
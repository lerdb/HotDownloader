//! 索引、监控快照和独立 MID 台账共用一个 SQLite 文件。
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::{path::Path, sync::Mutex};

pub type Result<T> = std::result::Result<T, String>;

pub struct Store(pub Mutex<Connection>);

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        let version: u64 = db
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if version > 1 {
            return Err("音乐库数据库版本较新，请使用对应版本的服务".into());
        }
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        // DELETE journal 在事务完成后只保留主文件；FULL 确保处理决定先落盘。
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS records (
                kind TEXT NOT NULL, key TEXT NOT NULL, data TEXT NOT NULL,
                PRIMARY KEY(kind,key));
            PRAGMA user_version=1;",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(db)))
    }

    pub fn get<T: DeserializeOwned>(&self, kind: &str, key: &str) -> Result<Option<T>> {
        read(&self.0.lock().unwrap(), kind, key)
    }

    pub fn all<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>> {
        let db = self.0.lock().unwrap();
        let mut stmt = db
            .prepare("SELECT data FROM records WHERE kind=? ORDER BY key")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kind], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }

    pub fn put<T: Serialize>(&self, kind: &str, key: &str, value: &T) -> Result<()> {
        write(&self.0.lock().unwrap(), kind, key, value)
    }

    pub fn insert<T: Serialize>(&self, kind: &str, key: &str, value: &T) -> Result<()> {
        let data = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.0
            .lock()
            .unwrap()
            .execute(
                "INSERT OR IGNORE INTO records(kind,key,data) VALUES(?,?,?)",
                params![kind, key, data],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 后台决策基于旧快照时，不能覆盖下载事件刚刚提交的新状态。
    pub fn compare_and_put<T: Serialize + DeserializeOwned>(
        &self,
        kind: &str,
        key: &str,
        expected: &T,
        value: &T,
    ) -> Result<bool> {
        let db = self.0.lock().unwrap();
        let current = read::<T>(&db, kind, key)?;
        if serde_json::to_value(current).map_err(|e| e.to_string())?
            != serde_json::to_value(expected).map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        write(&db, kind, key, value)?;
        Ok(true)
    }

    pub fn change<T: DeserializeOwned + Serialize>(
        &self,
        kind: &str,
        key: &str,
        f: impl FnOnce(&mut T),
    ) -> Result<()> {
        let db = self.0.lock().unwrap();
        if let Some(mut value) = read::<T>(&db, kind, key)? {
            let previous = serde_json::to_string(&value).map_err(|e| e.to_string())?;
            f(&mut value);
            if previous != serde_json::to_string(&value).map_err(|e| e.to_string())? {
                write(&db, kind, key, &value)?;
            }
        }
        Ok(())
    }
}

pub fn read<T: DeserializeOwned>(db: &Connection, kind: &str, key: &str) -> Result<Option<T>> {
    let raw: Option<String> = db
        .query_row(
            "SELECT data FROM records WHERE kind=? AND key=?",
            [kind, key],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    raw.map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        .transpose()
}

pub fn write<T: Serialize>(db: &Connection, kind: &str, key: &str, value: &T) -> Result<()> {
    let data = serde_json::to_string(value).map_err(|e| e.to_string())?;
    db.execute("INSERT INTO records(kind,key,data) VALUES(?,?,?) ON CONFLICT(kind,key) DO UPDATE SET data=excluded.data",
        params![kind, key, data]).map_err(|e| e.to_string())?;
    Ok(())
}

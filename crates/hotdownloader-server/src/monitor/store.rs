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
        if version > 2 {
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
            ",
        )
        .map_err(|e| e.to_string())?;
        if version < 2 {
            db.execute_batch("BEGIN;
                CREATE INDEX IF NOT EXISTS entry_state ON records(kind, json_extract(data,'$.state'), key);
                CREATE TABLE IF NOT EXISTS file_titles(title TEXT NOT NULL, path TEXT NOT NULL, PRIMARY KEY(title,path));
                CREATE TABLE IF NOT EXISTS file_issues(path TEXT PRIMARY KEY, blocks_matching INTEGER NOT NULL);
                CREATE INDEX IF NOT EXISTS file_issues_blocking ON file_issues(blocks_matching);
                CREATE TRIGGER IF NOT EXISTS file_delete AFTER DELETE ON records WHEN OLD.kind='file' BEGIN
                    DELETE FROM file_titles WHERE path=OLD.key;
                    DELETE FROM file_issues WHERE path=OLD.key;
                END;
                CREATE INDEX IF NOT EXISTS file_titles_path ON file_titles(path);
                CREATE TRIGGER IF NOT EXISTS file_insert AFTER INSERT ON records WHEN NEW.kind='file' BEGIN
                    INSERT INTO file_titles SELECT json_extract(NEW.data,'$.identity.title'),NEW.key
                    UNION SELECT json_extract(NEW.data,'$.metadata.title'),NEW.key
                    UNION SELECT json_extract(NEW.data,'$.filename.title'),NEW.key;
                    INSERT OR IGNORE INTO file_issues SELECT NEW.key,
                      (json_extract(NEW.data,'$.warning') IS NOT NULL OR json_extract(NEW.data,'$.identity.title')='' OR json_array_length(NEW.data,'$.identity.artists')=0) WHERE json_extract(NEW.data,'$.warning') IS NOT NULL
                      OR json_extract(NEW.data,'$.conflict')=1 OR json_extract(NEW.data,'$.identity.title')=''
                      OR json_array_length(NEW.data,'$.identity.artists')=0;
                END;
                CREATE TRIGGER IF NOT EXISTS file_update AFTER UPDATE OF data ON records WHEN NEW.kind='file' BEGIN
                    DELETE FROM file_titles WHERE path=NEW.key;
                    DELETE FROM file_issues WHERE path=NEW.key;
                    INSERT INTO file_titles SELECT json_extract(NEW.data,'$.identity.title'),NEW.key
                    UNION SELECT json_extract(NEW.data,'$.metadata.title'),NEW.key
                    UNION SELECT json_extract(NEW.data,'$.filename.title'),NEW.key;
                    INSERT OR IGNORE INTO file_issues SELECT NEW.key,
                      (json_extract(NEW.data,'$.warning') IS NOT NULL OR json_extract(NEW.data,'$.identity.title')='' OR json_array_length(NEW.data,'$.identity.artists')=0) WHERE json_extract(NEW.data,'$.warning') IS NOT NULL
                      OR json_extract(NEW.data,'$.conflict')=1 OR json_extract(NEW.data,'$.identity.title')=''
                      OR json_array_length(NEW.data,'$.identity.artists')=0;
                END;
                UPDATE records SET data=data WHERE kind='file';
                PRAGMA user_version=2;
                COMMIT;").map_err(|e| e.to_string())?;
        }
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
    db.execute("INSERT INTO records(kind,key,data) VALUES(?,?,?) ON CONFLICT(kind,key) DO UPDATE SET data=excluded.data WHERE records.data!=excluded.data",
        params![kind, key, data]).map_err(|e| e.to_string())?;
    Ok(())
}

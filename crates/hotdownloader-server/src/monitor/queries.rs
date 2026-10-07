use super::{
    library::{self, Identity, LocalFile, Match},
    store::{Result, Store},
    Entry, State,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct PageQuery {
    pub page: usize,
    pub page_size: usize,
    pub filter: String,
    pub query: String,
}
impl Default for PageQuery {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 30,
            filter: "all".into(),
            query: String::new(),
        }
    }
}
impl PageQuery {
    pub fn from_query(raw: &str) -> Result<Self> {
        let mut value = serde_json::Map::new();
        for pair in raw.split('&').filter(|part| !part.is_empty()) {
            let (key, raw) = pair.split_once('=').ok_or("查询参数无效")?;
            let mut bytes = Vec::new();
            let mut source = raw.bytes();
            while let Some(byte) = source.next() {
                bytes.push(match byte {
                    b'+' => b' ',
                    b'%' => {
                        let a = source
                            .next()
                            .and_then(|b| (b as char).to_digit(16))
                            .ok_or("查询编码无效")?;
                        let b = source
                            .next()
                            .and_then(|b| (b as char).to_digit(16))
                            .ok_or("查询编码无效")?;
                        (a * 16 + b) as u8
                    }
                    b => b,
                });
            }
            let text = String::from_utf8(bytes).map_err(|_| "查询编码无效")?;
            let field = if matches!(key, "page" | "pageSize") {
                json!(text.parse::<usize>().map_err(|_| "分页数字无效")?)
            } else {
                json!(text)
            };
            if value.insert(key.into(), field).is_some() {
                return Err("查询参数重复".into());
            }
        }
        let query: Self =
            serde_json::from_value(Value::Object(value)).map_err(|e| e.to_string())?;
        query.validate()?;
        Ok(query)
    }
    pub fn validate(&self) -> Result<()> {
        if self.page == 0 || !(1..=100).contains(&self.page_size) || self.query.len() > 300 {
            return Err(
                "分页参数无效：page 从 1 开始，pageSize 为 1–100，搜索最多 300 字节".into(),
            );
        }
        Ok(())
    }
}

impl Store {
    pub fn active_entries(&self) -> Result<Vec<Entry>> {
        let db = self.0.lock().unwrap();
        let mut stmt = db.prepare("SELECT data FROM records WHERE kind='entry' AND json_extract(data,'$.state') IN ('dispatching','queued','downloading','paused','interrupted','download_failed','credential_invalid','network_failed')").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn member_states(&self, members: &[String]) -> Result<Vec<(String, State)>> {
        let db = self.0.lock().unwrap();
        let mut stmt = db.prepare("SELECT r.key,json_extract(r.data,'$.state') FROM json_each(?) m JOIN records r ON r.kind='entry' AND r.key=m.value").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([json!(members).to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.map(|row| {
            let (mid, state) = row.map_err(|e| e.to_string())?;
            Ok((
                mid,
                serde_json::from_value(json!(state)).map_err(|e| e.to_string())?,
            ))
        })
        .collect()
    }

    pub fn pending_entries(&self, monitor: &super::Monitor) -> Result<Vec<Entry>> {
        let db = self.0.lock().unwrap();
        let mut stmt = db.prepare("SELECT r.data FROM json_each(?1) m JOIN records r ON r.kind='entry' AND r.key=m.value
            WHERE (?3 OR json_extract(r.data,'$.requestedBy')=?4) AND
            (json_extract(r.data,'$.state') IN ('pending','ready') OR
            (json_extract(r.data,'$.state') IN ('network_failed','download_failed','credential_invalid')
             AND json_extract(r.data,'$.owned')=1 AND json_extract(r.data,'$.retries')<3
             AND json_extract(r.data,'$.nextRetry') BETWEEN 1 AND ?2))
            ORDER BY r.key LIMIT 200").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params![
                    json!(monitor.members).to_string(),
                    super::now(),
                    monitor.config.enabled || monitor.draining,
                    monitor.id
                ],
                |r| r.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }

    pub fn song_page(&self, members: &[String], q: &PageQuery) -> Result<Value> {
        q.validate()?;
        let states = match q.filter.as_str() {
            "all" => vec![],
            "pending_confirmation" => vec!["pending_confirmation"],
            "active" => vec![
                "pending",
                "ready",
                "dispatching",
                "queued",
                "downloading",
                "paused",
                "interrupted",
            ],
            "done" => vec!["matched", "downloaded", "ignored"],
            "failed" => vec![
                "network_failed",
                "download_failed",
                "credential_invalid",
                "no_quality",
            ],
            _ => return Err("未知歌曲筛选条件".into()),
        };
        let db = self.0.lock().unwrap();
        let condition = "FROM json_each(?1) m JOIN records r ON r.kind='entry' AND r.key=m.value
            WHERE (?2='[]' OR json_extract(r.data,'$.state') IN (SELECT value FROM json_each(?2)))
            AND instr(lower(json_extract(r.data,'$.song.title') || ' ' || json_extract(r.data,'$.song.artist')),?3)>0";
        let members = json!(members).to_string();
        let states = json!(states).to_string();
        let search = q.query.trim().to_lowercase();
        let total: usize = db
            .query_row(
                &format!("SELECT count(*) {condition}"),
                params![members, states, search],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let page = q.page.min(total.div_ceil(q.page_size).max(1));
        let mut stmt = db
            .prepare(&format!(
                "SELECT r.data {condition} ORDER BY r.key LIMIT ?4 OFFSET ?5"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params![
                    members,
                    states,
                    search,
                    q.page_size,
                    (page - 1) * q.page_size
                ],
                |r| r.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?;
        let items = rows
            .map(|r| {
                serde_json::from_str::<Value>(&r.map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"items":items,"total":total,"page":page,"pageSize":q.page_size}))
    }

    pub fn issue_page(&self, q: &PageQuery) -> Result<Value> {
        q.validate()?;
        let db = self.0.lock().unwrap();
        let search = q.query.trim().to_lowercase();
        let total: usize = db
            .query_row(
                "SELECT count(*) FROM file_issues WHERE instr(lower(path),?)>0",
                [&search],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let page = q.page.min(total.div_ceil(q.page_size).max(1));
        let mut stmt = db.prepare("SELECT r.data FROM file_issues i JOIN records r ON r.kind='file' AND r.key=i.path WHERE instr(lower(i.path),?1)>0 ORDER BY i.path LIMIT ?2 OFFSET ?3").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params![search, q.page_size, (page - 1) * q.page_size],
                |r| r.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?;
        let items = rows
            .map(|r| {
                serde_json::from_str::<Value>(&r.map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"items":items,"total":total,"page":page,"pageSize":q.page_size}))
    }

    pub fn match_indexed(&self, identity: &Identity) -> Result<Match> {
        if identity.title.is_empty() || identity.artists.is_empty() {
            return Ok(Match::Confirm(vec![], "在线歌曲缺少标题或歌手".into()));
        }
        let (files, unresolved) = {
            let db = self.0.lock().unwrap();
            let mut stmt = db.prepare("SELECT r.data FROM file_titles t JOIN records r ON r.kind='file' AND r.key=t.path WHERE t.title=? ORDER BY t.path").map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([&identity.title], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let files = rows
                .map(|r| {
                    serde_json::from_str::<LocalFile>(&r.map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>>>()?;
            let unresolved: bool = db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM file_issues WHERE blocks_matching=1)",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            (files, unresolved)
        };
        Ok(match library::match_song(identity, &files) {
            Match::Missing if unresolved => Match::Confirm(
                vec![],
                "音乐库存在异常文件，请查看异常文件清单并确认是否下载".into(),
            ),
            other => other,
        })
    }
}

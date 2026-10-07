//! NAS/Web 的音乐库与歌单监控。下载仍通过共享 TaskService 调度。
pub mod library;
pub mod queries;
mod scheduler;
pub mod store;
#[cfg(test)]
mod tests;

use crate::runtime::{ServerEnvironment, ServerRuntime};
use futures_util::future::BoxFuture;
use hotdownloader_core::{
    platforms::qqmusic::{login, playlist},
    task::{
        contract::{
            CreateTaskRequest, CreateTaskResult, DuplicateAction, SongInput, TaskRecord, TaskStatus,
        },
        service::{TaskEnvironment, TaskService},
    },
};
use library::{Identity, LocalFile, Match, ScanReport};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use store::{Result, Store};
use tokio::sync::Mutex;

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
const BATCH_SIZE: usize = 20;
const RETRY_DELAYS: [u64; 3] = [300, 900, 3600];
const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRecord {
    pub started_at: u64,
    pub finished_at: u64,
    pub trigger: String,
    pub status: String,
    pub added: usize,
    pub linked: usize,
    pub enqueued: usize,
    pub failed: usize,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonitorInput {
    pub name: String,
    pub source: Source,
    #[serde(default)]
    pub playlist_id: String,
    #[serde(default)]
    pub dirid: String,
    pub quality: String,
    pub interval_minutes: u64,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Public,
    Created,
    Liked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: String,
    #[serde(flatten)]
    pub config: MonitorInput,
    pub members: Vec<String>,
    pub last_check: u64,
    pub next_check: u64,
    pub last_result: String,
    #[serde(default)]
    pub last_state: String,
    pub requested: bool,
    pub draining: bool,
    #[serde(default)]
    pub initial_members: Option<Vec<String>>,
    #[serde(default)]
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    Ready,
    Dispatching,
    Queued,
    Downloading,
    Paused,
    Interrupted,
    Matched,
    Downloaded,
    Ignored,
    PendingConfirmation,
    NoQuality,
    DownloadFailed,
    CredentialInvalid,
    NetworkFailed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub song: Value,
    pub identity: Identity,
    pub state: State,
    pub quality: String,
    pub task_id: Option<String>,
    pub owned: bool,
    pub path: Option<String>,
    pub candidates: Vec<LocalFile>,
    pub message: String,
    pub retries: usize,
    pub next_retry: u64,
    pub force_download: bool,
    #[serde(default)]
    pub dispatch_started_at: u64,
    #[serde(default)]
    pub requested_by: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchDecision {
    pub mids: Vec<String>,
    pub action: String,
}

fn same_playlist(a: &MonitorInput, b: &MonitorInput) -> bool {
    let liked = |m: &MonitorInput| {
        m.source == Source::Liked
            || (m.source == Source::Created && m.dirid.trim_start_matches('0') == "201")
    };
    if liked(a) && liked(b) {
        return true;
    }
    a.source == b.source
        && a.source != Source::Liked
        && a.playlist_id.trim_start_matches('0') == b.playlist_id.trim_start_matches('0')
        && (a.source != Source::Created
            || a.dirid.trim_start_matches('0') == b.dirid.trim_start_matches('0'))
}

impl Entry {
    fn mid(&self) -> &str {
        self.song["mid"].as_str().unwrap_or("")
    }
    fn fail(&mut self, state: State, message: String) {
        self.state = state;
        self.message = message;
        self.next_retry = RETRY_DELAYS
            .get(self.retries)
            .map(|delay| now() + delay)
            .unwrap_or(0);
    }
    fn terminal(&self) -> bool {
        matches!(
            self.state,
            State::Matched | State::Downloaded | State::Ignored
        )
    }
}

fn failure_state(message: &str) -> State {
    let text = message.to_lowercase();
    if [
        "超时",
        "网络",
        "timeout",
        "timed out",
        "network",
        "连接",
        "dns",
        "error sending request",
        "请求失败",
    ]
    .iter()
    .any(|part| text.contains(part))
    {
        State::NetworkFailed
    } else if [
        "凭据缺失",
        "凭据失效",
        "凭据无效",
        "登录凭证",
        "未登录",
        "登录过期",
        "重新登录",
        "认证失败",
    ]
    .iter()
    .any(|part| text.contains(part))
    {
        State::CredentialInvalid
    } else {
        State::DownloadFailed
    }
}

pub struct MonitorService {
    pub store: Arc<Store>,
    pub operation: Mutex<()>,
    pub scanning: Arc<AtomicBool>,
    pub running: AtomicUsize,
    scan_lock: Arc<Mutex<()>>,
    scan_generation: Arc<AtomicUsize>,
    monitor_locks: std::sync::Mutex<HashMap<String, std::sync::Weak<Mutex<()>>>>,
    reconcile_lock: Mutex<()>,
    // 台账写入失败时停止继续派发；重启后的对账会从持久化任务恢复。
    pub persistence_failed: AtomicBool,
    remote: Arc<dyn MonitorRemote>,
}

// 外部查询与调度分离，测试使用虚构歌单和凭据结果。
trait MonitorRemote: Send + Sync {
    fn fetch<'a>(
        &'a self,
        monitor: &'a Monitor,
        runtime: &'a ServerRuntime,
    ) -> BoxFuture<'a, Result<Vec<Value>>>;
    fn authenticate<'a>(&'a self, runtime: &'a ServerRuntime) -> BoxFuture<'a, Result<()>>;
}

struct QqRemote;
impl MonitorRemote for QqRemote {
    fn fetch<'a>(
        &'a self,
        monitor: &'a Monitor,
        runtime: &'a ServerRuntime,
    ) -> BoxFuture<'a, Result<Vec<Value>>> {
        Box::pin(async move {
            if monitor.config.source != Source::Public {
                self.authenticate(runtime).await?;
            }
            MonitorService::fetch(monitor, runtime).await
        })
    }
    fn authenticate<'a>(&'a self, runtime: &'a ServerRuntime) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if !login::check_credential_expired(runtime.login_store.as_ref()).await? {
                return Ok(());
            }
            login::refresh_credential(runtime.login_store.as_ref())
                .await
                .map(|_| ())
                .map_err(|error| {
                    if login::is_credential_rejection(&error)
                        || error.contains("登录凭证")
                        || error.contains("缺少刷新令牌")
                    {
                        format!("QQ 凭据失效，请重新登录: {error}")
                    } else {
                        error
                    }
                })
        })
    }
}

impl MonitorService {
    pub fn open(data_dir: &std::path::Path) -> Result<Arc<Self>> {
        Self::with_remote(data_dir, Arc::new(QqRemote))
    }

    fn with_remote(
        data_dir: &std::path::Path,
        remote: Arc<dyn MonitorRemote>,
    ) -> Result<Arc<Self>> {
        let store = Arc::new(Store::open(&data_dir.join("library.sqlite3"))?);
        for m in store.all::<Monitor>("monitor")? {
            store.change::<Vec<CheckRecord>>("history", &m.id, |history| {
                for record in history.iter_mut().filter(|r| r.status == "running") {
                    record.status = "interrupted".into();
                    record.message = "服务在本轮结束前退出，统计可能不完整".into();
                }
            })?;
        }
        Ok(Arc::new(Self {
            store,
            operation: Mutex::new(()),
            scanning: Arc::new(AtomicBool::new(false)),
            running: AtomicUsize::new(0),
            scan_lock: Arc::new(Mutex::new(())),
            scan_generation: Arc::new(AtomicUsize::new(0)),
            monitor_locks: std::sync::Mutex::new(HashMap::new()),
            reconcile_lock: Mutex::new(()),
            persistence_failed: AtomicBool::new(false),
            remote,
        }))
    }

    pub fn list(&self) -> Result<Value> {
        let monitors = self
            .store
            .all::<Monitor>("monitor")?
            .into_iter()
            .map(|m| {
                let states = self.store.member_states(&m.members)?;
                let mut counts: HashMap<String, usize> = HashMap::new();
                for (_, state) in &states {
                    let key = serde_json::to_value(state)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string();
                    *counts.entry(key).or_default() += 1;
                }
                let mut value = json!(m);
                value.as_object_mut().unwrap().remove("members");
                value.as_object_mut().unwrap().remove("initialMembers");
                value["memberCount"] = json!(m.members.len());
                value["counts"] = json!(counts);
                value["initialProgress"] = progress_from_states(&m, &states);
                value["latestRound"] = json!(self
                    .store
                    .get::<Vec<CheckRecord>>("history", &m.id)?
                    .and_then(|h| h.last().cloned()));
                Ok(value)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(
            json!({"monitors": monitors, "running": self.running.load(Ordering::Relaxed) > 0, "persistenceFailed": self.persistence_failed.load(Ordering::Relaxed)}),
        )
    }

    pub fn library_status(&self, env: &ServerEnvironment) -> Result<Value> {
        let mut report = self
            .store
            .get::<ScanReport>("config", "scan")?
            .unwrap_or_default();
        let config = env.download_config();
        report.roots = library::roots(
            env.default_download_dir(),
            &config.naming_template,
            &env.artist_separator(),
        )?;
        report.directories = report
            .roots
            .iter()
            .map(|root| {
                report
                    .directories
                    .iter()
                    .find(|d| d.path == root.path)
                    .cloned()
                    .unwrap_or_else(|| library::DirectoryStatus {
                        path: root.path.clone(),
                        state: "unknown".into(),
                        ..Default::default()
                    })
            })
            .collect();
        let mut value = json!(report);
        value["scanning"] = json!(self.scanning.load(Ordering::Relaxed));
        Ok(value)
    }

    pub async fn save(&self, id: Option<&str>, input: MonitorInput) -> Result<Monitor> {
        if input.name.trim().is_empty() || input.name.len() > 200 {
            return Err("请输入 1–200 字符的监控名称".into());
        }
        if !(5..=10080).contains(&input.interval_minutes) {
            return Err("检查间隔应为 5–10080 分钟".into());
        }
        let known =
            hotdownloader_core::task::rules::TaskRules::from_settings(&json!({})).quality_order;
        if !known.contains(&input.quality) {
            return Err("启用监控前请选择固定的自动下载音质".into());
        }
        if input.source != Source::Liked
            && (input.playlist_id.is_empty()
                || !input.playlist_id.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("请输入纯数字 QQ 歌单 ID".into());
        }
        if input.source == Source::Created
            && (input.dirid.is_empty() || !input.dirid.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("个人歌单需要有效目录 ID".into());
        }
        let _guard = self.operation.lock().await;
        if id.is_none() {
            if let Some(existing) = self
                .store
                .all::<Monitor>("monitor")?
                .iter()
                .find(|m| same_playlist(&m.config, &input))
            {
                return Err(format!(
                    "此歌单已有监控“{}”，请编辑或启用已有监控",
                    existing.config.name
                ));
            }
        }
        let mut monitor = if let Some(id) = id {
            let monitor = self
                .store
                .get::<Monitor>("monitor", id)?
                .ok_or("监控不存在")?;
            if monitor.config.source != input.source
                || monitor.config.playlist_id != input.playlist_id
                || monitor.config.dirid != input.dirid
            {
                return Err("已有监控不能更换歌单来源，请新增监控".into());
            }
            monitor
        } else {
            let id = format!(
                "{:x}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            Monitor {
                id,
                config: input.clone(),
                members: vec![],
                last_check: 0,
                next_check: 0,
                last_result: "等待首次检查".into(),
                last_state: "idle".into(),
                requested: false,
                draining: false,
                initial_members: None,
                revision: 0,
            }
        };
        let newly_enabled = input.enabled && (!monitor.config.enabled || monitor.last_check == 0);
        let interval_changed = monitor.config.interval_minutes != input.interval_minutes;
        monitor.config = input;
        monitor.revision += 1;
        if interval_changed {
            monitor.next_check = monitor.last_check + monitor.config.interval_minutes * 60;
        }
        if newly_enabled {
            monitor.next_check = 0;
        }
        if !monitor.config.enabled {
            monitor.draining = false;
            monitor.requested = false;
        }
        self.store.put("monitor", &monitor.id, &monitor)?;
        Ok(monitor)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let _guard = self.operation.lock().await;
        // 仅删除配置及成员快照；文件、任务与 MID 台账继续保留。
        let mut db = self.store.0.lock().unwrap();
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let deleted = tx
            .execute("DELETE FROM records WHERE kind='monitor' AND key=?", [id])
            .map_err(|e| e.to_string())?;
        if deleted == 0 {
            return Err("监控不存在".into());
        }
        tx.execute("DELETE FROM records WHERE kind='history' AND key=?", [id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn request_check(&self, id: &str) -> Result<()> {
        let _guard = self.operation.lock().await;
        let mut m = self
            .store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控不存在")?;
        m.requested = true;
        m.revision += 1;
        m.last_result = "已请求检查".into();
        m.last_state = "requested".into();
        self.store.put("monitor", id, &m)
    }

    #[cfg(test)]
    pub fn songs(&self, id: &str) -> Result<Value> {
        let monitor = self
            .store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控不存在")?;
        let entries: Vec<_> = self
            .store
            .all::<Entry>("entry")?
            .into_iter()
            .filter(|e| monitor.members.iter().any(|m| m == e.mid()))
            .collect();
        Ok(json!(entries))
    }

    pub fn song_page(&self, id: &str, query: &queries::PageQuery) -> Result<Value> {
        let monitor = self
            .store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控不存在")?;
        self.store.song_page(&monitor.members, query)
    }

    pub fn history(&self, id: &str) -> Result<Value> {
        self.store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控不存在")?;
        let mut history = self
            .store
            .get::<Vec<CheckRecord>>("history", id)?
            .unwrap_or_default();
        history.reverse();
        Ok(json!(history))
    }

    /// 任务事件在持久化后同步进入台账，浏览器立即删除任务也不会丢失成功决定。
    pub fn observe_task(&self, task: &TaskRecord) -> Result<()> {
        if task.platform != "qqmusic" {
            return Ok(());
        }
        // 手动下载也记录成功 MID，之后新增监控或清除任务时仍然记得下载决定。
        if task.status == TaskStatus::Completed
            && self.store.get::<Entry>("entry", &task.song_mid)?.is_none()
        {
            let entry = Entry {
                song: json!({"platform":"qqmusic", "id":task.song_id, "mid":task.song_mid, "title":task.song_title,
                    "artist":task.artist, "album":task.album, "coverUrl":task.cover_url, "mediaMid":task.media_mid,
                    "qualities":task.available_qualities.clone().unwrap_or_default()}),
                identity: Identity::new(&task.song_title, [task.artist.clone()]),
                state: State::Downloaded,
                quality: task.quality.clone(),
                task_id: Some(task.id.clone()),
                owned: false,
                path: task.file_path.clone(),
                candidates: vec![],
                message: "已下载，保留处理决定".into(),
                retries: 0,
                next_retry: 0,
                force_download: false,
                dispatch_started_at: 0,
                requested_by: None,
            };
            self.store.insert("entry", &task.song_mid, &entry)?;
        }
        self.store.change::<Entry>("entry", &task.song_mid, |e| {
            if e.terminal() {
                return;
            }
            // 歌曲等待匹配/确认期间的手动下载成功，也应抑制后续自动补齐。
            if task.status == TaskStatus::Completed
                && !e.force_download
                && e.task_id.as_deref() != Some(&task.id)
            {
                e.task_id = Some(task.id.clone());
                e.owned = false;
            }
            if e.task_id.as_deref() != Some(&task.id) {
                if e.state != State::Dispatching
                    || e.task_id.is_some()
                    || task.added_at < e.dispatch_started_at
                {
                    return;
                }
                e.task_id = Some(task.id.clone());
            }
            e.quality = task.quality.clone();
            match task.status {
                TaskStatus::Completed => {
                    e.state = State::Downloaded;
                    e.path = task.file_path.clone();
                    e.message = "下载成功，已记入处理台账".into();
                    e.next_retry = 0;
                }
                TaskStatus::Waiting => {
                    e.state = State::Queued;
                    e.message = "已进入下载队列".into();
                }
                TaskStatus::Downloading | TaskStatus::Processing => {
                    e.state = State::Downloading;
                }
                TaskStatus::Paused => {
                    e.state = State::Paused;
                }
                TaskStatus::Interrupted if e.state != State::Paused => {
                    e.state = State::Interrupted;
                }
                TaskStatus::Error
                    if !matches!(
                        e.state,
                        State::DownloadFailed
                            | State::CredentialInvalid
                            | State::NetworkFailed
                            | State::NoQuality
                    ) =>
                {
                    let message = task.error_msg.clone().unwrap_or_else(|| "下载失败".into());
                    let state = failure_state(&message);
                    e.fail(state, message);
                    if task.error_msg.as_deref().is_some_and(|message| {
                        message
                            .starts_with(hotdownloader_core::download::fallback::QUALITY_EXHAUSTED)
                    }) {
                        e.state = State::NoQuality;
                        e.next_retry = 0;
                    }
                }
                _ => {}
            }
        })
    }

    pub async fn scan(&self, runtime: &ServerRuntime) -> Result<ScanReport> {
        self.scan_cached(runtime, true).await
    }

    async fn scan_cached(&self, runtime: &ServerRuntime, force: bool) -> Result<ScanReport> {
        let generation = self.scan_generation.load(Ordering::Relaxed);
        let scan_guard = self.scan_lock.clone().lock_owned().await;
        let config = runtime.environment.download_config();
        let roots = library::roots(
            runtime.environment.default_download_dir(),
            &config.naming_template,
            &runtime.environment.artist_separator(),
        )?;
        if let Some(report) = self.store.get::<ScanReport>("config", "scan")? {
            if report.roots == roots
                && report.last_scan.saturating_add(30) > now()
                && (!force || generation != self.scan_generation.load(Ordering::Relaxed))
            {
                if let Some(error) = &report.error {
                    return Err(error.clone());
                }
            }
            if report.indexed_roots == roots
                && report.error.is_none()
                && report.last_success.saturating_add(300) > now()
                && (!force || generation != self.scan_generation.load(Ordering::Relaxed))
            {
                let checked = report.clone();
                if let Err(error) =
                    tokio::task::spawn_blocking(move || library::check_cached_roots(&checked))
                        .await
                        .map_err(|e| e.to_string())?
                {
                    let mut failed = report;
                    failed.error = Some(error.clone());
                    failed.last_scan = now();
                    for directory in &mut failed.directories {
                        if error.contains(&directory.path) {
                            directory.state = "unavailable".into();
                            directory.error = Some(error.clone());
                            directory.checked_at = now();
                        }
                    }
                    self.store.put("config", "scan", &failed)?;
                    return Err(error);
                }
                return Ok(report);
            }
        }
        let excluded = runtime
            .tasks
            .list()
            .into_iter()
            .filter(|t| t.status != TaskStatus::Completed)
            .filter_map(|t| t.save_path)
            .map(|p| {
                std::path::Path::new(&p)
                    .canonicalize()
                    .unwrap_or_else(|_| p.clone().into())
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        self.scanning.store(true, Ordering::Relaxed);
        let store = self.store.clone();
        let scanning = self.scanning.clone();
        let generation = self.scan_generation.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = scan_guard;
            struct Reset(Arc<AtomicBool>);
            impl Drop for Reset {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::Relaxed);
                }
            }
            let _reset = Reset(scanning);
            let result = library::scan(&store, roots, excluded);
            generation.fetch_add(1, Ordering::Relaxed);
            result
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// 快照和所有新 MID 在同一事务写入；歌单移除只更新成员关系。
    pub fn ingest(&self, monitor: &mut Monitor, songs: Vec<Value>, separator: &str) -> Result<()> {
        let mut db = self.store.0.lock().unwrap();
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let mut members = Vec::new();
        let mut seen_members = HashSet::new();
        for mut song in songs {
            if !song.is_object() {
                return Err("歌单包含无效歌曲数据，保留上次快照".into());
            }
            song["platform"] = json!("qqmusic");
            let parsed: SongInput = serde_json::from_value(song.clone())
                .map_err(|e| format!("歌单歌曲数据不完整: {e}"))?;
            if parsed.mid.is_empty() || !parsed.mid.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return Err("歌单包含无效 MID，保留上次快照".into());
            }
            if !seen_members.insert(parsed.mid.clone()) {
                continue;
            }
            members.push(parsed.mid.clone());
            let artists = song["artists"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|a| a["name"].as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| parsed.artist.split(separator).map(str::to_string).collect());
            if let Some(mut entry) = store::read::<Entry>(&tx, "entry", &parsed.mid)? {
                // 保留决定和重试预算，只刷新在线字段，手动重下可使用最新音质列表。
                entry.identity = Identity::new(&parsed.title, artists);
                entry.song = song;
                store::write(&tx, "entry", &parsed.mid, &entry)?;
            } else {
                let entry = Entry {
                    identity: Identity::new(&parsed.title, artists),
                    song,
                    state: State::Pending,
                    quality: monitor.config.quality.clone(),
                    task_id: None,
                    owned: true,
                    path: None,
                    candidates: vec![],
                    message: "等待目录匹配".into(),
                    retries: 0,
                    next_retry: 0,
                    force_download: false,
                    dispatch_started_at: 0,
                    requested_by: None,
                };
                store::write(&tx, "entry", &parsed.mid, &entry)?;
            }
        }
        if monitor.initial_members.is_none() {
            monitor.initial_members = Some(members.clone());
        }
        monitor.members = members;
        monitor.last_check = now();
        monitor.next_check = now() + monitor.config.interval_minutes * 60;
        monitor.requested = false;
        monitor.draining = true;
        monitor.last_result = format!("已检查 {} 首歌曲，按批次补齐", monitor.members.len());
        monitor.last_state = "checked".into();
        store::write(&tx, "monitor", &monitor.id, monitor)?;
        tx.commit().map_err(|e| e.to_string())
    }

    async fn fetch(monitor: &Monitor, runtime: &ServerRuntime) -> Result<Vec<Value>> {
        let separator = runtime.environment.artist_separator();
        let raw = match monitor.config.source {
            Source::Public => {
                playlist::fetch_playlist_songs(&separator, monitor.config.playlist_id.clone())
                    .await?
            }
            Source::Created => {
                playlist::fetch_created_playlist_songs(
                    runtime.login_store.as_ref(),
                    &separator,
                    monitor.config.playlist_id.clone(),
                    monitor.config.dirid.clone(),
                )
                .await?
            }
            Source::Liked => {
                let lists: Value = serde_json::from_str(
                    &playlist::fetch_created_playlists(runtime.login_store.as_ref()).await?,
                )
                .map_err(|e| e.to_string())?;
                let liked = lists["playlists"]
                    .as_array()
                    .and_then(|items| items.iter().find(|p| p["dirid"] == "201"))
                    .ok_or("当前账号未返回“我喜欢”歌单，请检查 QQ 登录状态")?;
                playlist::fetch_created_playlist_songs(
                    runtime.login_store.as_ref(),
                    &separator,
                    liked["id"].as_str().ok_or("我喜欢歌单 ID 缺失")?.into(),
                    "201".into(),
                )
                .await?
            }
        };
        let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let songs = value["songs"]
            .as_array()
            .ok_or("歌单响应缺少歌曲列表")?
            .clone();
        // 不接受截断的歌单，防止缺失项被当成已移除。
        if value["playlist"]["songCount"]
            .as_u64()
            .is_some_and(|count| count > songs.len() as u64)
        {
            return Err("歌单返回不完整，保留上次快照".into());
        }
        Ok(songs)
    }

    pub async fn decide(
        &self,
        runtime: &ServerRuntime,
        mid: &str,
        action: &str,
        path: Option<String>,
        monitor_id: Option<&str>,
    ) -> Result<()> {
        if action == "refresh" {
            return self
                .refresh_candidates(
                    runtime,
                    monitor_id.ok_or("请提供当前监控的 monitorId")?,
                    mid,
                )
                .await;
        }
        let verified_file = if action == "link" {
            let file = self
                .store
                .get::<LocalFile>("file", path.as_deref().ok_or("请选择现有文件")?)?
                .ok_or("文件不在当前索引中，请重新扫描")?;
            let candidate = file.clone();
            tokio::task::spawn_blocking(move || library::verify_file(&candidate))
                .await
                .map_err(|e| e.to_string())??;
            Some(file)
        } else {
            None
        };
        let _guard = self.operation.lock().await;
        let _task_guard = runtime.tasks.creation_lock.lock().await;
        if let Some(file) = verified_file {
            if self.store.get::<LocalFile>("file", &file.path)?.as_ref() != Some(&file) {
                return Err("文件索引已变化，请重新扫描后确认".into());
            }
        }
        let monitors = self.store.all::<Monitor>("monitor")?;
        let monitor = if let Some(id) = monitor_id {
            let monitor = monitors.iter().find(|m| m.id == id).ok_or("监控不存在")?;
            if !monitor.members.iter().any(|s| s == mid) {
                return Err("歌曲不属于当前监控，请刷新后重试".into());
            }
            Some(monitor)
        } else {
            // 兼容单一归属的旧客户端；共享歌曲必须明确操作来源。
            let mut owners = monitors
                .iter()
                .filter(|m| m.members.iter().any(|s| s == mid));
            let monitor = owners.next();
            if owners.next().is_some() || monitor.is_none() {
                return Err("请提供当前监控的 monitorId".into());
            }
            monitor
        };
        let mut entry = self.store.get::<Entry>("entry", mid)?.ok_or("歌曲不存在")?;
        self.prepare_decision(runtime, &mut entry, action, path, monitor)?;
        self.store.put("entry", mid, &entry)
    }

    pub async fn decide_batch(
        &self,
        runtime: &ServerRuntime,
        id: &str,
        input: BatchDecision,
    ) -> Result<usize> {
        if !matches!(input.action.as_str(), "download" | "ignore") {
            return Err("批量操作仅支持下载或忽略待确认歌曲".into());
        }
        if input.mids.is_empty() || input.mids.len() > 200 {
            return Err("每次请选择 1–200 首待确认歌曲".into());
        }
        let _guard = self.operation.lock().await;
        let _task_guard = runtime.tasks.creation_lock.lock().await;
        let monitor = self
            .store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控不存在")?;
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for mid in &input.mids {
            if !seen.insert(mid) {
                continue;
            }
            if !monitor.members.contains(mid) {
                return Err("所选歌曲不属于当前监控，请刷新后重试".into());
            }
            let mut entry = self.store.get::<Entry>("entry", mid)?.ok_or("歌曲不存在")?;
            if entry.state != State::PendingConfirmation {
                return Err("所选歌曲状态已变化，请刷新后重新选择待确认歌曲".into());
            }
            self.prepare_decision(runtime, &mut entry, &input.action, None, Some(&monitor))?;
            entries.push(entry);
        }
        // 任务事件可能在校验期间更新台账；事务内再次比较，整批成功或整批不写入。
        let mut db = self.store.0.lock().unwrap();
        let tx = db.transaction().map_err(|e| e.to_string())?;
        for entry in &entries {
            let current = store::read::<Entry>(&tx, "entry", entry.mid())?.ok_or("歌曲不存在")?;
            if current.state != State::PendingConfirmation {
                return Err("所选歌曲状态已变化，请刷新后重试".into());
            }
            store::write(&tx, "entry", entry.mid(), entry)?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(entries.len())
    }

    fn prepare_decision(
        &self,
        runtime: &ServerRuntime,
        entry: &mut Entry,
        action: &str,
        path: Option<String>,
        monitor: Option<&Monitor>,
    ) -> Result<()> {
        let mid = entry.mid();
        if runtime.tasks.list().iter().any(|t| {
            t.platform == "qqmusic"
                && t.song_mid == mid
                && !matches!(t.status, TaskStatus::Completed | TaskStatus::Error)
        }) {
            return Err("此歌曲仍有未结束的下载任务，请先在任务页处理".into());
        }
        if matches!(
            entry.state,
            State::Dispatching
                | State::Queued
                | State::Downloading
                | State::Paused
                | State::Interrupted
        ) {
            return Err("请先在任务页结束现有任务，再修改处理决定".into());
        }
        match action {
            "retry" => {
                if !matches!(
                    entry.state,
                    State::NetworkFailed
                        | State::DownloadFailed
                        | State::CredentialInvalid
                        | State::NoQuality
                ) {
                    return Err("仅失败歌曲可以重试".into());
                }
                if let Some(id) = &entry.task_id {
                    if !entry.owned {
                        return Err("此任务由任务页管理，请打开任务后重试".into());
                    }
                    let task = runtime
                        .tasks
                        .get(id)
                        .ok_or("任务记录已移除，请清除决定并重新下载")?;
                    if task.status != TaskStatus::Error {
                        return Err("任务状态已变化，请刷新后重试".into());
                    }
                    runtime.tasks.update(id, true, |task| {
                        task.retry_count = 0;
                    })?;
                }
                entry.state = State::Ready;
                entry.retries = 0;
                entry.next_retry = 0;
                entry.force_download = true;
                entry.message = "已请求重试，本次仍最多自动重试 3 次".into();
                if let Some(m) = monitor {
                    entry.requested_by = Some(m.id.clone());
                    if entry.task_id.is_none() {
                        entry.quality = m.config.quality.clone();
                    }
                }
            }
            "link" => {
                let path = path.ok_or("请选择现有文件")?;
                if runtime.tasks.path_reserved(&path) {
                    return Err("文件正在被下载任务使用，请稍后再关联".into());
                }
                entry.state = State::Matched;
                entry.path = Some(path);
                entry.message = "已手动关联本地文件".into();
            }
            "ignore" => {
                entry.state = State::Ignored;
                entry.message = "已手动忽略".into();
            }
            "download" | "reset" => {
                entry.state = State::Ready;
                entry.task_id = None;
                entry.owned = true;
                entry.path = None;
                entry.retries = 0;
                entry.next_retry = 0;
                entry.force_download = true;
                entry.message = "已请求下载".into();
                // 只派发明确选择的歌曲，不开启停用歌单的整轮补齐。
                if let Some(m) = monitor {
                    entry.quality = m.config.quality.clone();
                    entry.requested_by = Some(m.id.clone());
                }
            }
            _ => return Err("未知逐曲操作".into()),
        }
        entry.candidates.clear();
        // 只做状态变更，实际派发由同一个服务循环完成。
        Ok(())
    }

    async fn refresh_candidates(&self, runtime: &ServerRuntime, id: &str, mid: &str) -> Result<()> {
        let monitor = self
            .store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控不存在")?;
        if !monitor.members.iter().any(|member| member == mid) {
            return Err("歌曲不属于当前监控".into());
        }
        let previous = self.store.get::<Entry>("entry", mid)?.ok_or("歌曲不存在")?;
        if previous.state != State::PendingConfirmation {
            return Err("仅待确认歌曲可以刷新候选".into());
        }
        self.scan(runtime).await?;
        let store = self.store.clone();
        let identity = previous.identity.clone();
        let matched = tokio::task::spawn_blocking(move || store.match_indexed(&identity))
            .await
            .map_err(|e| e.to_string())??;
        let _guard = self.operation.lock().await;
        let current = self
            .store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控已删除")?;
        if !current.members.iter().any(|member| member == mid) {
            return Err("歌曲已移出监控".into());
        }
        let mut entry = previous.clone();
        entry.candidates = match matched {
            Match::Found(path) => self
                .store
                .get::<LocalFile>("file", &path)?
                .into_iter()
                .collect(),
            Match::Confirm(files, _) => files,
            Match::Missing => vec![],
        };
        entry.message = if entry.candidates.is_empty() {
            "候选已刷新，未找到可关联文件；请决定下载或忽略"
        } else {
            "候选已刷新，请选择文件关联，或决定下载、忽略"
        }
        .into();
        if !self
            .store
            .compare_and_put("entry", mid, &previous, &entry)?
        {
            return Err("歌曲状态已变化，请刷新后重试".into());
        }
        Ok(())
    }

    pub async fn reconcile(&self, runtime: &ServerRuntime) -> Result<()> {
        let tasks = runtime.tasks.list();
        for task in tasks.iter().filter(|t| t.status == TaskStatus::Completed) {
            self.observe_task(task)?;
        }
        let service = TaskService::new(
            &runtime.tasks,
            &runtime.engine,
            runtime.environment.as_ref(),
        );
        for mut entry in self.store.active_entries()? {
            if entry.terminal() {
                continue;
            }
            // 补齐 JSON 任务写入与台账回调之间的崩溃窗口。
            let task = entry
                .task_id
                .as_ref()
                .and_then(|id| tasks.iter().find(|t| &t.id == id))
                .or_else(|| {
                    if entry.state == State::Dispatching {
                        tasks
                            .iter()
                            .filter(|t| {
                                t.platform == "qqmusic"
                                    && t.song_mid == entry.mid()
                                    && t.added_at >= entry.dispatch_started_at
                            })
                            .max_by_key(|t| t.added_at)
                    } else {
                        None
                    }
                });
            if let Some(task) = task {
                self.observe_task(task)?;
                if entry.owned
                    && entry.state != State::Paused
                    && task.status == TaskStatus::Interrupted
                {
                    service.resume_task(task.id.clone()).await?;
                }
            } else if entry.task_id.is_some() || entry.state == State::Dispatching {
                let previous = entry.clone();
                entry.state = State::PendingConfirmation;
                entry.message = "任务记录已移除或派发中断，请确认后再下载".into();
                entry.task_id = None;
                entry.next_retry = 0;
                self.store
                    .compare_and_put("entry", entry.mid(), &previous, &entry)?;
            }
        }
        Ok(())
    }

    async fn dispatch(
        &self,
        runtime: &ServerRuntime,
        mut entry: Entry,
        auth: &Result<()>,
    ) -> Result<bool> {
        let mid = entry.mid().to_string();
        let previous = entry.clone();
        let service = TaskService::new(
            &runtime.tasks,
            &runtime.engine,
            runtime.environment.as_ref(),
        );
        if !entry.force_download && entry.task_id.is_none() {
            if let Some(task) = runtime
                .tasks
                .list()
                .iter()
                .filter(|t| t.platform == "qqmusic" && t.song_mid == mid)
                .max_by_key(|t| {
                    (
                        !matches!(t.status, TaskStatus::Completed | TaskStatus::Error),
                        t.added_at,
                        t.id.clone(),
                    )
                })
            {
                entry.task_id = Some(task.id.clone());
                entry.owned = false;
                if !self
                    .store
                    .compare_and_put("entry", &mid, &previous, &entry)?
                {
                    return Ok(false);
                }
                self.observe_task(task)?;
                return Ok(false);
            }
        }
        let song: SongInput =
            serde_json::from_value(entry.song.clone()).map_err(|e| e.to_string())?;
        if runtime
            .environment
            .task_rules()
            .initial_quality(&entry.quality, &song.qualities)
            .is_none()
        {
            entry.state = State::NoQuality;
            entry.message = "所选音质及允许的降级音质均不可用".into();
            entry.next_retry = 0;
            self.store
                .compare_and_put("entry", &mid, &previous, &entry)?;
            return Ok(false);
        }
        if matches!(
            entry.state,
            State::DownloadFailed | State::CredentialInvalid | State::NetworkFailed
        ) {
            entry.retries += 1;
        }
        if let Err(error) = auth {
            entry.fail(failure_state(error), error.clone());
            self.store
                .compare_and_put("entry", &mid, &previous, &entry)?;
            return Ok(false);
        }
        entry.state = State::Dispatching;
        entry.dispatch_started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if !self
            .store
            .compare_and_put("entry", &mid, &previous, &entry)?
        {
            return Ok(false);
        }
        if let Some(id) = &entry.task_id {
            if !entry.owned {
                return Ok(false);
            }
            match service.retry_task(id.clone()).await {
                Ok(true) => return Ok(true),
                Ok(false) => self.store.change::<Entry>("entry", &mid, |e| {
                    e.state = State::DownloadFailed;
                    e.next_retry = 0;
                    e.message = "下载引擎无法重试，请手动处理".into();
                })?,
                Err(error) => self
                    .store
                    .change::<Entry>("entry", &mid, |e| e.fail(failure_state(&error), error))?,
            }
        } else {
            // 自动模式强制 ask，防止浏览器的覆盖/跳过偏好替代逐曲确认。
            let environment = AutomaticEnvironment(runtime.environment.as_ref());
            let service = TaskService::new(&runtime.tasks, &runtime.engine, &environment);
            match service
                .create_automatic_task(
                    CreateTaskRequest {
                        song,
                        desired_quality: entry.quality,
                        duplicate_action: if entry.force_download {
                            Some(DuplicateAction::Rename)
                        } else {
                            None
                        },
                    },
                    entry.force_download,
                )
                .await
            {
                Ok(CreateTaskResult::Created { task }) => {
                    self.observe_task(&task)?;
                    return Ok(true);
                }
                Ok(CreateTaskResult::Existing { task }) => {
                    self.store.change::<Entry>("entry", &mid, |e| {
                        e.task_id = Some(task.id.clone());
                        e.owned = false;
                    })?;
                    // 复用后重新读取，避免覆盖等待创建锁期间已经到达的完成事件。
                    if let Some(current) = runtime.tasks.get(&task.id) {
                        self.observe_task(&current)?;
                    }
                    return Ok(false);
                }
                Ok(_) => self.store.change::<Entry>("entry", &mid, |e| {
                    e.state = State::PendingConfirmation;
                    e.message = "下载目标路径已存在，请确认处理方式".into();
                })?,
                Err(error) => self
                    .store
                    .change::<Entry>("entry", &mid, |e| e.fail(failure_state(&error), error))?,
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
fn initial_progress(monitor: &Monitor, entries: &[Entry]) -> Value {
    progress_from_states(
        monitor,
        &entries
            .iter()
            .map(|e| (e.mid().to_string(), e.state.clone()))
            .collect::<Vec<_>>(),
    )
}

fn progress_from_states(monitor: &Monitor, entries: &[(String, State)]) -> Value {
    let Some(members) = &monitor.initial_members else {
        return json!({"initialized":false});
    };
    let current: HashSet<_> = monitor.members.iter().map(String::as_str).collect();
    let states: HashMap<_, _> = entries
        .iter()
        .map(|(mid, state)| (mid.as_str(), state))
        .collect();
    let mut completed: usize = 0;
    let mut removed = 0;
    let mut confirmation = 0;
    let mut failed = 0;
    let mut active = 0;
    for mid in members {
        if !current.contains(mid.as_str()) {
            removed += 1;
            continue;
        }
        if let Some(e) = states.get(mid.as_str()) {
            if matches!(e, State::Matched | State::Downloaded | State::Ignored) {
                completed += 1;
            } else if **e == State::PendingConfirmation {
                confirmation += 1;
            } else if matches!(
                e,
                State::NoQuality
                    | State::DownloadFailed
                    | State::CredentialInvalid
                    | State::NetworkFailed
            ) {
                failed += 1;
            } else {
                active += 1;
            }
        } else {
            active += 1;
        }
    }
    let total = members.len();
    json!({"initialized":true, "total":total, "completed":completed, "removed":removed,
        "confirmation":confirmation, "failed":failed, "active":active,
        "percent": ((completed + removed) * 100).checked_div(total).unwrap_or(100)})
}

fn eligible(entry: &Entry) -> bool {
    matches!(entry.state, State::Pending | State::Ready)
        || (entry.owned
            && matches!(
                entry.state,
                State::DownloadFailed | State::CredentialInvalid | State::NetworkFailed
            )
            && entry.retries < RETRY_DELAYS.len()
            && entry.next_retry > 0
            && entry.next_retry <= now())
}

struct AutomaticEnvironment<'a>(&'a ServerEnvironment);
impl TaskEnvironment for AutomaticEnvironment<'_> {
    fn task_rules(&self) -> hotdownloader_core::task::rules::TaskRules {
        let mut rules = self.0.task_rules();
        rules.duplicate_strategy = "ask".into();
        rules
    }
    fn download_config(&self) -> hotdownloader_core::download::config::DownloadConfig {
        self.0.download_config()
    }
    fn file_exists(&self, path: &str, is_saf: bool, uri: Option<&str>) -> bool {
        self.0.file_exists(path, is_saf, uri)
    }
    fn file_len(&self, path: &str, is_saf: bool) -> Option<u64> {
        self.0.file_len(path, is_saf)
    }
}

pub fn start(runtime: &Arc<ServerRuntime>) {
    let weak = Arc::downgrade(runtime);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(runtime) = weak.upgrade() else { break };
            tokio::spawn(async move {
                if let Err(error) = runtime.monitors.tick(&runtime).await {
                    log::warn!("歌单监控: {error}");
                }
            });
        }
    });
}

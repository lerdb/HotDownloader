//! NAS/Web 的音乐库与歌单监控。下载仍通过共享 TaskService 调度。
pub mod library;
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
        atomic::{AtomicBool, Ordering},
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

pub struct MonitorService {
    pub store: Arc<Store>,
    pub operation: Mutex<()>,
    pub scanning: AtomicBool,
    pub running: AtomicBool,
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
            let auth = login::download_auth(runtime.login_store.as_ref()).await;
            if auth.auth.is_none() || auth.refresh_error.is_some() {
                Err("QQ 凭据缺失或失效，请在设置中重新登录".into())
            } else {
                Ok(())
            }
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
        Ok(Arc::new(Self {
            store: Arc::new(Store::open(&data_dir.join("library.sqlite3"))?),
            operation: Mutex::new(()),
            scanning: AtomicBool::new(false),
            running: AtomicBool::new(false),
            persistence_failed: AtomicBool::new(false),
            remote,
        }))
    }

    pub fn list(&self) -> Result<Value> {
        let entries = self.store.all::<Entry>("entry")?;
        let monitors = self
            .store
            .all::<Monitor>("monitor")?
            .into_iter()
            .map(|m| {
                let mut counts: HashMap<String, usize> = HashMap::new();
                for entry in entries
                    .iter()
                    .filter(|e| m.members.iter().any(|mid| mid == e.mid()))
                {
                    let key = serde_json::to_value(&entry.state)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string();
                    *counts.entry(key).or_default() += 1;
                }
                let mut value = json!(m);
                value["counts"] = json!(counts);
                value
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"monitors": monitors, "running": self.running.load(Ordering::Relaxed), "persistenceFailed": self.persistence_failed.load(Ordering::Relaxed)}),
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
            }
        };
        let newly_enabled = input.enabled && (!monitor.config.enabled || monitor.last_check == 0);
        let interval_changed = monitor.config.interval_minutes != input.interval_minutes;
        monitor.config = input;
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

    pub async fn request_check(&self, id: &str) -> Result<()> {
        let _guard = self.operation.lock().await;
        let mut m = self
            .store
            .get::<Monitor>("monitor", id)?
            .ok_or("监控不存在")?;
        m.requested = true;
        m.last_result = "已请求检查".into();
        m.last_state = "requested".into();
        self.store.put("monitor", id, &m)
    }

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
                    if e.state != State::DownloadFailed && e.state != State::CredentialInvalid =>
                {
                    let message = task.error_msg.clone().unwrap_or_else(|| "下载失败".into());
                    let state = if message.contains("凭据") || message.contains("登录") {
                        State::CredentialInvalid
                    } else {
                        State::DownloadFailed
                    };
                    e.fail(state, message);
                }
                _ => {}
            }
        })
    }

    pub async fn scan(&self, runtime: &ServerRuntime) -> Result<ScanReport> {
        let config = runtime.environment.download_config();
        let roots = library::roots(
            runtime.environment.default_download_dir(),
            &config.naming_template,
            &runtime.environment.artist_separator(),
        )?;
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
        let result = tokio::task::spawn_blocking(move || library::scan(&store, roots, excluded))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
        self.scanning.store(false, Ordering::Relaxed);
        result
    }

    /// 快照和所有新 MID 在同一事务写入；歌单移除只更新成员关系。
    pub fn ingest(&self, monitor: &mut Monitor, songs: Vec<Value>, separator: &str) -> Result<()> {
        let mut db = self.store.0.lock().unwrap();
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let mut members = Vec::new();
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
            if members.contains(&parsed.mid) {
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
                };
                store::write(&tx, "entry", &parsed.mid, &entry)?;
            }
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
    ) -> Result<()> {
        let _guard = self.operation.lock().await;
        let _task_guard = runtime.tasks.creation_lock.lock().await;
        let mut entry = self.store.get::<Entry>("entry", mid)?.ok_or("歌曲不存在")?;
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
            "link" => {
                let path = path.ok_or("请选择现有文件")?;
                let file = self
                    .store
                    .get::<LocalFile>("file", &path)?
                    .ok_or("文件不在当前索引中，请重新扫描")?;
                if !std::path::Path::new(&file.path).is_file() {
                    return Err("文件已不可访问，请重新扫描".into());
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
                // 当前监控配置决定手动重下的品质；MID 决定跨歌单共享。
                let mut monitors = self.store.all::<Monitor>("monitor")?;
                if let Some(m) = monitors
                    .iter_mut()
                    .find(|m| m.members.iter().any(|s| s == mid))
                {
                    entry.quality = m.config.quality.clone();
                    m.draining = true;
                    self.store.put("monitor", &m.id, m)?;
                }
            }
            _ => return Err("未知逐曲操作".into()),
        }
        entry.candidates.clear();
        self.store.put("entry", mid, &entry)?;
        // 只做状态变更，实际派发由同一个服务循环完成。
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
        for mut entry in self.store.all::<Entry>("entry")? {
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
    ) -> Result<()> {
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
                .find(|t| t.platform == "qqmusic" && t.song_mid == mid)
            {
                entry.task_id = Some(task.id.clone());
                entry.owned = false;
                if !self
                    .store
                    .compare_and_put("entry", &mid, &previous, &entry)?
                {
                    return Ok(());
                }
                self.observe_task(task)?;
                return Ok(());
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
            return Ok(());
        }
        if matches!(
            entry.state,
            State::DownloadFailed | State::CredentialInvalid
        ) {
            entry.retries += 1;
        }
        if let Err(error) = auth {
            entry.fail(State::CredentialInvalid, error.clone());
            self.store
                .compare_and_put("entry", &mid, &previous, &entry)?;
            return Ok(());
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
            return Ok(());
        }
        if let Some(id) = &entry.task_id {
            if !entry.owned {
                return Ok(());
            }
            match service.retry_task(id.clone()).await {
                Ok(true) => {}
                Ok(false) => self.store.change::<Entry>("entry", &mid, |e| {
                    e.state = State::DownloadFailed;
                    e.next_retry = 0;
                    e.message = "下载引擎无法重试，请手动处理".into();
                })?,
                Err(error) => self
                    .store
                    .change::<Entry>("entry", &mid, |e| e.fail(State::DownloadFailed, error))?,
            }
        } else {
            // 自动模式强制 ask，防止浏览器的覆盖/跳过偏好替代逐曲确认。
            let environment = AutomaticEnvironment(runtime.environment.as_ref());
            let service = TaskService::new(&runtime.tasks, &runtime.engine, &environment);
            match service
                .create_download_task(CreateTaskRequest {
                    song,
                    desired_quality: entry.quality,
                    duplicate_action: if entry.force_download {
                        Some(DuplicateAction::Rename)
                    } else {
                        None
                    },
                })
                .await
            {
                Ok(CreateTaskResult::Created { task }) => self.observe_task(&task)?,
                Ok(_) => self.store.change::<Entry>("entry", &mid, |e| {
                    e.state = State::PendingConfirmation;
                    e.message = "下载目标路径已存在，请确认处理方式".into();
                })?,
                Err(error) => self
                    .store
                    .change::<Entry>("entry", &mid, |e| e.fail(State::DownloadFailed, error))?,
            }
        }
        Ok(())
    }

    pub async fn tick(&self, runtime: &ServerRuntime) -> Result<()> {
        let _guard = self.operation.lock().await;
        if self.persistence_failed.load(Ordering::Relaxed) {
            return Err("台账持久化失败，停止自动下载；修复存储后重启服务".into());
        }
        self.reconcile(runtime).await?;
        let mut monitors = self.store.all::<Monitor>("monitor")?;
        let due = monitors
            .iter()
            .any(|m| m.requested || (m.config.enabled && m.next_check <= now()));
        let active: HashSet<String> = monitors
            .iter()
            .filter(|m| m.config.enabled || m.draining)
            .flat_map(|m| m.members.clone())
            .collect();
        let pending = self
            .store
            .all::<Entry>("entry")?
            .iter()
            .any(|e| active.contains(e.mid()) && eligible(e));
        if !due && !pending {
            return Ok(());
        }
        if let Err(error) = self.scan(runtime).await {
            for m in monitors
                .iter_mut()
                .filter(|m| m.config.enabled || m.requested || m.draining)
            {
                m.last_result = format!("扫描失败，本轮补齐已跳过: {error}");
                m.last_state = "scan_failed".into();
                self.store.put("monitor", &m.id, m)?;
            }
            return Err(error);
        }
        for m in monitors
            .iter_mut()
            .filter(|m| m.requested || (m.config.enabled && m.next_check <= now()))
        {
            let result =
                tokio::time::timeout(Duration::from_secs(120), self.remote.fetch(m, runtime))
                    .await
                    .map_err(|_| "读取歌单超时".to_string())
                    .and_then(|r| r);
            let result = result
                .and_then(|songs| self.ingest(m, songs, &runtime.environment.artist_separator()));
            if let Err(error) = result {
                m.last_check = now();
                m.next_check = now() + m.config.interval_minutes * 60;
                m.requested = false;
                m.last_result = format!("歌单检查失败: {error}");
                m.last_state = if error.contains("凭据") || error.contains("登录") {
                    "credential_invalid".into()
                } else {
                    "check_failed".into()
                };
                self.store.put("monitor", &m.id, m)?;
            }
        }
        let active: HashSet<String> = monitors
            .iter()
            .filter(|m| m.config.enabled || m.draining)
            .flat_map(|m| m.members.clone())
            .collect();
        let files = self.store.all::<LocalFile>("file")?;
        let mut dispatched = 0;
        let mut auth = None;
        for mut entry in self.store.all::<Entry>("entry")? {
            if !active.contains(entry.mid()) || !eligible(&entry) {
                continue;
            }
            if entry.state == State::Pending
                || (entry.state == State::Ready && !entry.force_download)
            {
                let previous = entry.clone();
                match library::match_song(&entry.identity, &files) {
                    Match::Found(path) => {
                        entry.state = State::Matched;
                        entry.path = Some(path);
                        entry.message = "已匹配本地文件".into();
                    }
                    Match::Missing => {
                        entry.state = State::Ready;
                        entry.message = "等待分批入队".into();
                    }
                    Match::Confirm(candidates, reason) => {
                        entry.state = State::PendingConfirmation;
                        entry.candidates = candidates;
                        entry.message = reason;
                    }
                }
                if !self
                    .store
                    .compare_and_put("entry", entry.mid(), &previous, &entry)?
                {
                    continue;
                }
            }
            if eligible(&entry) && dispatched < BATCH_SIZE {
                if auth.is_none() {
                    auth = Some(
                        tokio::time::timeout(
                            Duration::from_secs(45),
                            self.remote.authenticate(runtime),
                        )
                        .await
                        .map_err(|_| "QQ 凭据校验超时".to_string())
                        .and_then(|r| r),
                    );
                }
                self.dispatch(runtime, entry, auth.as_ref().unwrap())
                    .await?;
                dispatched += 1;
            }
        }
        Ok(())
    }
}

fn eligible(entry: &Entry) -> bool {
    matches!(entry.state, State::Pending | State::Ready)
        || (entry.owned
            && matches!(
                entry.state,
                State::DownloadFailed | State::CredentialInvalid
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
            runtime.monitors.running.store(true, Ordering::Relaxed);
            if let Err(error) = runtime.monitors.tick(&runtime).await {
                log::warn!("歌单监控: {error}");
            }
            runtime.monitors.running.store(false, Ordering::Relaxed);
        }
    });
}

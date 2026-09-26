use crate::contract::{
    BatchResult, CreateTaskRequest, CreateTaskResult, DuplicateAction, TaskRecord, TaskStatus,
};
use crate::download_config::DownloadConfig;
use crate::engine::{DownloadEngine, ERR_TASK_CONTEXT_MISSING};
use crate::platform::Platform;
use crate::task_context::SongInfo;
use crate::task_rules::TaskRules;
use crate::task_state::TaskState;

/// 运行时只提供设置快照和文件系统查询。任务的创建、冲突处理和重试规则留在核心。
/// SAF 的路径解释由 Tauri 实现；普通服务端只需检查本地文件。
pub trait TaskEnvironment: Send + Sync {
    fn task_rules(&self) -> TaskRules;
    fn download_config(&self) -> DownloadConfig;
    fn file_exists(&self, path: &str, is_saf: bool, saf_folder_uri: Option<&str>) -> bool;
    fn file_len(&self, path: &str, is_saf: bool) -> Option<u64>;
}

/// 每个命令临时借用运行时管理的状态与引擎，不持有窗口或 IPC 句柄。
pub struct TaskService<'a> {
    state: &'a TaskState,
    engine: &'a DownloadEngine,
    environment: &'a dyn TaskEnvironment,
}

/// 路径检查使用一次配置快照，重命名时继续使用同一个 SAF 目录。
struct PathCheck {
    original_path: String,
    exists: bool,
    suggested_path: String,
    is_saf: bool,
    saf_folder_uri: Option<String>,
}

impl<'a> TaskService<'a> {
    pub fn new(
        state: &'a TaskState,
        engine: &'a DownloadEngine,
        environment: &'a dyn TaskEnvironment,
    ) -> Self {
        Self {
            state,
            engine,
            environment,
        }
    }

    // 创建、控制、重试都由这里编排。页面关闭后下载器仍依靠 TaskState
    // 和自身的调度循环继续工作，HTTP 与 IPC 入口都不拥有任务。
    pub fn load_tasks(&self) -> Vec<TaskRecord> {
        self.state.list()
    }

    async fn register_engine_task(&self, task: &TaskRecord) -> Result<(), String> {
        // 持久化记录是下载器上下文的来源。重试旧任务时可用同一记录重建上下文，
        // 无需依赖前端页面保留 URL、路径或下载参数。
        let platform = Platform::from_str(&task.platform)?;
        self.engine
            .add_task(
                task.id.clone(),
                platform,
                task.song_id,
                task.song_mid.clone(),
                String::new(),
                task.save_path.clone().unwrap_or_default(),
                task.quality.clone(),
                task.filename.clone(),
                String::new(),
                task.file_size,
                task.song_title.clone(),
                task.artist.clone(),
                task.album.clone(),
                task.cover_url.clone(),
                task.downloaded,
            )
            .await;
        Ok(())
    }

    async fn resume_offset(&self, task: &TaskRecord) -> u64 {
        // 内存中的 downloaded 可能来自上次进度事件，断点续传必须以实际文件长度为准。
        // SAF 路径不能直接用 std::fs 读取；此处回退到 0，让下载器从头处理。
        let path = match task.save_path.as_deref() {
            Some(path) if std::path::Path::new(path).is_absolute() => Some(path.to_string()),
            _ => self
                .task_path_check(task)
                .await
                .ok()
                .filter(|result| !result.is_saf)
                .map(|result| result.original_path),
        };
        // 文件大小超过预期总量时不能继续追加，否则可能损坏目标文件。
        path.and_then(|path| self.environment.file_len(&path, false))
            .filter(|size| task.file_size == 0 || *size <= task.file_size)
            .unwrap_or(0)
    }

    fn unique_path(
        &self,
        original: &str,
        suggested: &str,
        is_saf: bool,
        saf_folder_uri: Option<&str>,
    ) -> Result<String, String> {
        // 同时检查磁盘与尚未创建文件的排队任务，避免两个任务取得同一目标。
        if !self.state.path_reserved(suggested)
            && !self
                .environment
                .file_exists(suggested, is_saf, saf_folder_uri)
        {
            return Ok(suggested.to_string());
        }
        let path = std::path::Path::new(original);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("无效文件名")?;
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        for n in 1..10_000 {
            let name = if ext.is_empty() {
                format!("{stem} ({n})")
            } else {
                format!("{stem} ({n}).{ext}")
            };
            let candidate = if is_saf {
                name
            } else {
                path.parent()
                    .ok_or("无效下载目录")?
                    .join(name)
                    .to_string_lossy()
                    .to_string()
            };
            if !self.state.path_reserved(&candidate)
                && !self
                    .environment
                    .file_exists(&candidate, is_saf, saf_folder_uri)
            {
                return Ok(candidate);
            }
        }
        Err("无法生成不重复的文件名".into())
    }

    pub async fn create_download_task(
        &self,
        request: CreateTaskRequest,
    ) -> Result<CreateTaskResult, String> {
        Platform::from_str(&request.song.platform)?;
        let state = self.state;
        // 锁覆盖路径检查、占用和入队，防止并发创建时绕过重名判断。
        let _creation_guard = state.creation_lock.lock().await;
        let rules = self.environment.task_rules();
        let selected = rules
            .initial_quality(&request.desired_quality, &request.song.qualities)
            .cloned();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis() as u64;
        let id = format!("{now:x}-{:x}", rand::random::<u64>());
        // 即使音质不可用，也建立可见的错误任务，供用户查看失败原因。
        let mut task = TaskRecord {
            id,
            platform: request.song.platform,
            song_id: request.song.id,
            song_mid: request.song.mid,
            song_title: request.song.title,
            artist: request.song.artist,
            album: request.song.album,
            cover_url: request.song.cover_url,
            media_mid: request.song.media_mid,
            filename: selected
                .as_ref()
                .map_or_else(String::new, |q| q.filename.clone()),
            quality: selected
                .as_ref()
                .map_or(request.desired_quality, |q| q.quality.clone()),
            status: if selected.is_some() {
                TaskStatus::Waiting
            } else {
                TaskStatus::Error
            },
            error_msg: if selected.is_some() {
                None
            } else {
                Some("所选音质不可用".into())
            },
            file_path: None,
            file_size: selected.as_ref().map_or(0, |q| q.size),
            downloaded: 0,
            retry_count: 0,
            added_at: now,
            available_qualities: Some(request.song.qualities),
            speed: None,
            save_path: None,
        };

        if selected.is_some() {
            let path = self.task_path_check(&task).await?;
            let reserved = state.path_reserved(&path.original_path);
            if path.exists || reserved {
                // 请求中的显式选择优先于默认策略；ask 返回结果让 UI 弹窗，
                // 用户决定后再调用同一个命令，不能由 UI 自己计算重命名路径。
                let action = request
                    .duplicate_action
                    .or(match rules.duplicate_strategy.as_str() {
                        "overwrite" => Some(DuplicateAction::Overwrite),
                        "rename" => Some(DuplicateAction::Rename),
                        "cancel" => Some(DuplicateAction::Cancel),
                        _ => None,
                    });
                match action {
                    None => {
                        return Ok(CreateTaskResult::NeedsConfirmation {
                            song_title: task.song_title,
                        })
                    }
                    Some(DuplicateAction::Cancel) => return Ok(CreateTaskResult::Cancelled),
                    Some(DuplicateAction::Rename) => {
                        task.save_path = Some(self.unique_path(
                            &path.original_path,
                            &path.suggested_path,
                            path.is_saf,
                            path.saf_folder_uri.as_deref(),
                        )?)
                    }
                    Some(DuplicateAction::Overwrite) if reserved => {
                        // 正在使用的目标文件不能覆盖，否则会破坏另一任务的下载。
                        return Err("同名任务正在下载，请选择保留两份".into());
                    }
                    Some(DuplicateAction::Overwrite) => task.save_path = Some(path.original_path),
                }
            } else {
                task.save_path = Some(path.original_path);
            }
        }

        // 先持久化，再交给下载器；窗口关闭或注册失败时仍能看到任务及错误。
        state.insert(task.clone())?;
        if task.status == TaskStatus::Waiting {
            if let Err(e) = self.register_engine_task(&task).await {
                state.failed(&task.id, &e, None);
                return Err(e);
            }
        }
        Ok(CreateTaskResult::Created { task })
    }

    pub async fn pause_task(&self, task_id: String) -> Result<(), String> {
        let state = self.state;
        let _creation_guard = state.creation_lock.lock().await;
        let task = state.get(&task_id).ok_or("任务不存在")?;
        if task.status != TaskStatus::Downloading && task.status != TaskStatus::Waiting {
            return Err("只有等待或下载中的任务可以暂停".into());
        }
        // 先让下载器停止工作，再更新权威状态，避免 UI 先显示暂停但仍在写文件。
        self.engine.pause(&task_id).await;
        state.update(&task_id, true, |task| {
            task.status = TaskStatus::Paused;
            task.speed = None;
        })?;
        Ok(())
    }

    pub async fn resume_task(&self, task_id: String) -> Result<(), String> {
        let state = self.state;
        let _creation_guard = state.creation_lock.lock().await;
        let task = state.get(&task_id).ok_or("任务不存在")?;
        match task.status {
            TaskStatus::Paused => {
                // 同一进程中的暂停保留了下载器上下文，直接恢复运行即可。
                self.engine.resume(&task_id).await;
                state.update(&task_id, true, |task| task.status = TaskStatus::Downloading)?;
            }
            TaskStatus::Interrupted => {
                // 进程重启后的下载器上下文已经消失；按磁盘文件长度恢复原品质任务。
                // 此操作保留 retry_count，下载错误的重试与降级仍由 retry_task 处理。
                self.engine.wait_for_task_exit(&task_id).await;
                let offset = self.resume_offset(&task).await;
                let resumed = state.update(&task_id, true, |record| {
                    record.status = TaskStatus::Waiting;
                    record.downloaded = offset;
                    record.speed = None;
                    record.error_msg = None;
                })?;
                if let Err(error) = self.register_engine_task(&resumed).await {
                    state.failed(&task_id, &format!("启动下载失败: {error}"), None);
                    return Err(error);
                }
            }
            _ => return Err("只有暂停或中断的任务可以恢复".into()),
        }
        Ok(())
    }

    pub async fn cancel_task(&self, task_id: String, delete_file: bool) -> Result<(), String> {
        let engine = self.engine;
        let state = self.state;
        let _creation_guard = state.creation_lock.lock().await;
        let task = state.get(&task_id).ok_or("任务不存在")?;
        // 已完成任务可能不在下载器活动表中，删除文件时仍须把最终路径交给引擎。
        if let Some(path) = task.file_path.as_deref().or_else(|| {
            task.save_path
                .as_deref()
                .filter(|path| std::path::Path::new(path).is_absolute())
        }) {
            engine.remember_final_path(&task_id, path).await;
        }
        if let Err(e) = engine.remove(&task_id, delete_file).await {
            if task.status != TaskStatus::Completed {
                state.failed(&task_id, &e, None);
            }
            return Err(e);
        }
        state.remove(&task_id)
    }

    pub async fn remove_task(&self, task_id: String, delete_file: bool) -> Result<(), String> {
        self.cancel_task(task_id, delete_file).await
    }

    /// 批量移除任务。用于“清除所选”等场景，
    /// 避免前端逐个 invoke（每个任务一次 IPC + 一次整表写盘）导致的卡顿。
    pub async fn remove_tasks(
        &self,
        task_ids: Vec<String>,
        delete_file: bool,
    ) -> Result<BatchResult, String> {
        let mut errors: Vec<String> = Vec::new();
        // 让前端获得后端真实成功/失败数量，用于清除完成后的准确通知。
        let mut succeeded: usize = 0;
        let mut failed: usize = 0;
        for task_id in task_ids {
            if let Err(e) = self.cancel_task(task_id.clone(), delete_file).await {
                log::error!("批量移除任务失败 {}: {}", task_id, e);
                // 批量移除失败时保留 task_id，便于前端/日志定位具体失败任务。
                errors.push(format!("{}: {}", task_id, e));
                failed += 1;
            } else {
                succeeded += 1;
            }
        }
        Ok(BatchResult {
            succeeded,
            failed,
            errors,
        })
    }

    pub async fn retry_task(&self, task_id: String) -> Result<bool, String> {
        let state = self.state;
        let _creation_guard = state.creation_lock.lock().await;
        let mut task = state.get(&task_id).ok_or("任务不存在")?;
        if task.status != TaskStatus::Error {
            return Ok(false);
        }
        let engine = self.engine;
        // 上一轮 worker 可能刚发出失败事件但尚未退出；先等待，避免新旧 worker 并发写入。
        engine.wait_for_task_exit(&task_id).await;
        let rules = self.environment.task_rules();
        let next_count = task.retry_count + 1;
        let mut downgraded = false;
        if next_count > 3 {
            // 次数耗尽后的行为完全由 Rust 决定；旧任务没有品质列表时不能凭空推断文件名。
            if !rules.auto_downgrade {
                state.update(&task_id, true, |t| {
                    t.retry_count = next_count;
                    t.error_msg = Some("重试次数已用尽".into());
                })?;
                return Ok(false);
            }
            let available = match task.available_qualities.as_deref() {
                Some(available) => available,
                None => {
                    state.update(&task_id, true, |t| {
                        t.retry_count = next_count;
                        t.error_msg = Some("旧任务缺少可用品质信息，请重新添加下载".into());
                    })?;
                    return Ok(false);
                }
            };
            let next = match rules.next_quality(&task.quality, available) {
                Some(next) => next.clone(),
                None => {
                    state.update(&task_id, true, |t| {
                        t.retry_count = next_count;
                        t.error_msg = Some("已无后续可用品质".into());
                    })?;
                    return Ok(false);
                }
            };
            task.quality = next.quality;
            task.filename = next.filename;
            task.file_size = next.size;
            task.downloaded = 0;
            task.retry_count = 0;
            task.save_path = None;
            downgraded = true;
            // 降级后的文件名和大小都已变更，需要重新计算路径并解决冲突。
            let path = self.task_path_check(&task).await?;
            let reserved = state.path_reserved(&path.original_path);
            if path.exists || reserved {
                match rules.duplicate_strategy.as_str() {
                    "cancel" => {
                        state.update(&task_id, true, |t| {
                            t.error_msg = Some("降级目标文件已存在".into());
                        })?;
                        return Ok(false);
                    }
                    "overwrite" if !reserved => task.save_path = Some(path.original_path),
                    // 重试可能由批量操作触发，不能等待浏览器弹窗；ask 时自动保留两份。
                    _ => {
                        task.save_path = Some(self.unique_path(
                            &path.original_path,
                            &path.suggested_path,
                            path.is_saf,
                            path.saf_folder_uri.as_deref(),
                        )?)
                    }
                }
            } else {
                task.save_path = Some(path.original_path);
            }
        } else {
            // 同品质重试保留目标路径，并从磁盘重新测量已下载字节数。
            task.retry_count = next_count;
            task.downloaded = self.resume_offset(&task).await;
        }

        // 先提交等待状态及本次重试参数，前端随后只接收状态快照。
        state.update(&task_id, true, |record| {
            record.quality = task.quality.clone();
            record.filename = task.filename.clone();
            record.file_size = task.file_size;
            record.downloaded = task.downloaded;
            record.retry_count = task.retry_count;
            record.save_path = task.save_path.clone();
            record.error_msg = None;
            record.status = TaskStatus::Waiting;
        })?;

        let result = if downgraded {
            // 品质改变后，旧下载器上下文中的 URL、目标路径都不再适用。
            engine.forget_final_path(&task_id).await;
            self.register_engine_task(&task).await
        } else {
            match engine.enqueue_task(&task_id, task.downloaded).await {
                Ok(()) => Ok(()),
                Err(e) if e.contains(ERR_TASK_CONTEXT_MISSING) => {
                    // 应用重启后只有持久化记录，没有进程内下载器上下文，直接重建。
                    self.register_engine_task(&task).await
                }
                Err(e) => Err(e),
            }
        };
        if let Err(e) = result {
            state.failed(&task_id, &format!("启动下载失败: {e}"), None);
            return Err(e);
        }
        Ok(true)
    }

    pub fn set_max_concurrent(&self, max: u32) -> Result<(), String> {
        self.engine.set_concurrency(max);
        Ok(())
    }

    async fn task_path_check(&self, task: &TaskRecord) -> Result<PathCheck, String> {
        // quality 参与命名模板，创建任务与 worker 必须使用相同的 SongInfo。
        let song_info = SongInfo {
            title: task.song_title.clone(),
            artist: task.artist.clone(),
            album: task.album.clone(),
            quality: task.quality.clone(),
            cover_url: task.cover_url.clone(),
        };
        // 路径检查与 worker 使用同一套核心配置规则，避免创建和实际下载选择不同的目录。
        let config = self.environment.download_config();
        let (is_saf, download_dir, saf_folder_uri) = crate::download_path::resolve_download_path(
            &config.download_dir,
            &config.naming_template,
            config.saf_folder_uri.as_deref(),
            &song_info,
            &task.filename,
        );

        // 路径解析交给运行时。Android 传入相对文件名，普通文件系统传入完整路径。
        let exists = self
            .environment
            .file_exists(&download_dir, is_saf, saf_folder_uri.as_deref());
        // 建议路径仅供冲突处理；排队任务的路径占用稍后由 unique_path 再次检查。
        if exists {
            let path = std::path::Path::new(&download_dir);
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed");
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            for counter in 1..10_000 {
                let new_name = if ext.is_empty() {
                    format!("{} ({})", stem, counter)
                } else {
                    format!("{} ({}).{}", stem, counter, ext)
                };
                let new_path = if is_saf {
                    new_name.clone()
                } else {
                    path.parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(&new_name)
                        .to_string_lossy()
                        .to_string()
                };
                let new_exists =
                    self.environment
                        .file_exists(&new_path, is_saf, saf_folder_uri.as_deref());
                if !new_exists {
                    // 路径结果在服务层直接使用类型，避免为了内部调用序列化再反序列化。
                    return Ok(PathCheck {
                        original_path: download_dir,
                        exists,
                        suggested_path: new_path,
                        is_saf,
                        saf_folder_uri,
                    });
                }
            }
            return Err("无法生成不重复的文件名".into());
        }

        // 文件不存在时，suggested_path 返回原始路径
        Ok(PathCheck {
            original_path: download_dir.clone(),
            exists,
            suggested_path: download_dir,
            is_saf,
            saf_folder_uri,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures_util::future::BoxFuture;

    use super::{TaskEnvironment, TaskService};
    use crate::contract::{
        CreateTaskRequest, CreateTaskResult, DuplicateAction, QualityItem, SongInput, TaskRecord,
    };
    use crate::download_config::DownloadConfig;
    use crate::engine::{DownloadEngine, DownloadTaskRunner, TaskController};
    use crate::local_file_deleter::LocalFileDeleter;
    use crate::ports::NoopCompletionNotifier;
    use crate::task_context::TaskContext;
    use crate::task_rules::TaskRules;
    use crate::task_state::{TaskEventSink, TaskRepository, TaskState};

    struct MemoryRepository(Mutex<Vec<TaskRecord>>);

    impl TaskRepository for MemoryRepository {
        fn load(&self) -> Result<Vec<TaskRecord>, String> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn save(&self, tasks: &[TaskRecord]) -> Result<(), String> {
            *self.0.lock().unwrap() = tasks.to_vec();
            Ok(())
        }
    }

    struct QuietEvents;

    impl TaskEventSink for QuietEvents {
        fn updated(&self, _task: TaskRecord) {}
        fn removed(&self, _task_id: &str) {}
    }

    struct QuietRunner;

    impl DownloadTaskRunner for QuietRunner {
        fn run(
            &self,
            _context: TaskContext,
            _controller: TaskController,
        ) -> BoxFuture<'static, bool> {
            Box::pin(async { true })
        }

        fn report_error(&self, _task_id: &str, _message: &str) {}
    }

    struct ExistingOriginalPath {
        download_dir: String,
        file_length: Option<u64>,
    }

    impl TaskEnvironment for ExistingOriginalPath {
        fn task_rules(&self) -> TaskRules {
            TaskRules::from_settings(&serde_json::json!({ "duplicateStrategy": "ask" }))
        }

        fn download_config(&self) -> DownloadConfig {
            DownloadConfig::from_settings(&serde_json::json!({}), &self.download_dir)
        }

        fn file_exists(&self, path: &str, _is_saf: bool, _saf_folder_uri: Option<&str>) -> bool {
            // 模拟原名已存在，建议的新路径没有文件。
            path.ends_with("歌曲 - 歌手.mp3")
        }

        fn file_len(&self, _path: &str, _is_saf: bool) -> Option<u64> {
            self.file_length
        }
    }

    fn request(action: Option<DuplicateAction>) -> CreateTaskRequest {
        CreateTaskRequest {
            song: SongInput {
                platform: "qqmusic".into(),
                id: 1,
                mid: "song-mid".into(),
                title: "歌曲".into(),
                artist: "歌手".into(),
                album: String::new(),
                cover_url: String::new(),
                media_mid: String::new(),
                qualities: vec![QualityItem {
                    quality: "320kmp3".into(),
                    filename: "source.mp3".into(),
                    size: 100,
                }],
            },
            desired_quality: "320kmp3".into(),
            duplicate_action: action,
        }
    }

    #[tokio::test]
    async fn duplicate_confirmation_and_rename_stay_in_rust_service() {
        let repository = Arc::new(MemoryRepository(Mutex::new(Vec::new())));
        let state = TaskState::load(repository.clone(), Arc::new(QuietEvents)).unwrap();
        let engine = DownloadEngine::new(
            Arc::new(QuietRunner),
            Arc::new(LocalFileDeleter),
            Arc::new(NoopCompletionNotifier),
        );
        let environment = ExistingOriginalPath {
            download_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            file_length: None,
        };
        let service = TaskService::new(&state, &engine, &environment);

        // ask 只返回待确认结果，不占用任务 ID 或写入任务表。
        let first = service.create_download_task(request(None)).await.unwrap();
        assert!(matches!(first, CreateTaskResult::NeedsConfirmation { .. }));
        assert!(state.list().is_empty());

        // 前端只传用户选择；实际新文件名和任务持久化都在同一服务内完成。
        let second = service
            .create_download_task(request(Some(DuplicateAction::Rename)))
            .await
            .unwrap();
        let CreateTaskResult::Created { task } = second else {
            panic!("重命名应创建任务");
        };
        assert!(task.save_path.unwrap().ends_with("歌曲 - 歌手 (1).mp3"));
        assert_eq!(repository.load().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn interrupted_task_resumes_from_file_without_spending_retry_budget() {
        let mut record: TaskRecord = serde_json::from_value(serde_json::json!({
            "id": "interrupted-1", "platform": "qqmusic", "songId": 1, "songMid": "mid",
            "songTitle": "歌曲", "artist": "歌手", "album": "专辑",
            "filename": "song.mp3", "quality": "320kmp3", "status": "interrupted",
            "fileSize": 100, "downloaded": 0, "retryCount": 3, "addedAt": 1
        }))
        .unwrap();
        record.save_path = Some(
            std::env::temp_dir()
                .join("interrupted-1.mp3")
                .to_string_lossy()
                .into_owned(),
        );
        let repository = Arc::new(MemoryRepository(Mutex::new(vec![record])));
        let state = TaskState::load(repository.clone(), Arc::new(QuietEvents)).unwrap();
        let engine = DownloadEngine::new(
            Arc::new(QuietRunner),
            Arc::new(LocalFileDeleter),
            Arc::new(NoopCompletionNotifier),
        );
        let environment = ExistingOriginalPath {
            download_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            file_length: Some(40),
        };
        let service = TaskService::new(&state, &engine, &environment);

        // 已达到普通错误的降级阈值；中断恢复仍保持原品质和计数。
        assert!(!service.retry_task("interrupted-1".into()).await.unwrap());
        service.resume_task("interrupted-1".into()).await.unwrap();
        let resumed = state.get("interrupted-1").unwrap();
        assert_eq!(resumed.status, crate::contract::TaskStatus::Waiting);
        assert_eq!(resumed.retry_count, 3);
        assert_eq!(resumed.quality, "320kmp3");
        assert_eq!(resumed.downloaded, 40);
    }
}

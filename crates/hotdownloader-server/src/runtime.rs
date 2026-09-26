//! 独立进程组装共享核心。此模块持有任务和调度器，HTTP 连接只借用它们。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use futures_util::future::BoxFuture;
use hotdownloader_core::contract::{TaskRecord, TaskStatus};
use hotdownloader_core::download_config::{DownloadConfig, DownloadConfigProvider};
use hotdownloader_core::download_link::PlatformDownloadLinkProvider;
use hotdownloader_core::download_worker::download_task;
use hotdownloader_core::engine::{DownloadEngine, DownloadTaskRunner, TaskController};
use hotdownloader_core::json_task_repository::JsonTaskRepository;
use hotdownloader_core::local_download_file::LocalDownloadFileOpener;
use hotdownloader_core::local_file_deleter::LocalFileDeleter;
use hotdownloader_core::ports::{DownloadProgressSink, NoopCompletionNotifier};
use hotdownloader_core::postprocess::LocalDownloadPostprocessor;
use hotdownloader_core::qq_login::FileLoginStore;
use hotdownloader_core::settings_patch::{
    apply_patch, snapshot, SettingsPatch, SettingsPatchError, SettingsScope, SettingsSnapshot,
};
use hotdownloader_core::task_context::TaskContext;
use hotdownloader_core::task_rules::TaskRules;
use hotdownloader_core::task_service::TaskEnvironment;
use hotdownloader_core::task_state::{TaskEventSink, TaskState};
use serde_json::Value;
use tokio::sync::broadcast;

/// SSE 与任务操作共用一个事件总线。发送端没有订阅者时仍照常执行任务。
#[derive(Clone, Debug)]
pub struct ServerEvent {
    pub name: &'static str,
    pub data: String,
}

pub struct ServerEvents {
    sender: broadcast::Sender<ServerEvent>,
}

impl ServerEvents {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(512);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }

    pub fn send(&self, name: &'static str, data: String) {
        // 没有浏览器订阅是正常情况；下载任务不能依赖事件发送结果。
        let _ = self.sender.send(ServerEvent { name, data });
    }
}

impl TaskEventSink for ServerEvents {
    fn updated(&self, task: TaskRecord) {
        // 高频进度只走 SSE；稳定的完成和错误状态才写入容器日志。
        match task.status {
            TaskStatus::Completed => log::info!("任务 {} 下载完成", task.id),
            TaskStatus::Error => log::warn!(
                "任务 {} 失败: {}",
                task.id,
                task.error_msg.as_deref().unwrap_or("未知原因")
            ),
            _ => {}
        }
        if let Ok(data) = serde_json::to_string(&task) {
            self.send("task-updated", data);
        }
    }

    fn removed(&self, task_id: &str) {
        log::info!("任务 {} 已删除", task_id);
        self.send("task-removed", serde_json::json!(task_id).to_string());
    }
}

/// 配置从独立服务的数据目录读取一次。任务启动时仍得到自己的配置快照。
pub struct ServerEnvironment {
    settings: RwLock<Value>,
    settings_path: PathBuf,
    default_download_dir: String,
}

impl ServerEnvironment {
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("settings.json");
        let settings = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str::<Value>(&raw)
                .map_err(|error| format!("解析 settings.json 失败: {error}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                serde_json::json!({})
            }
            Err(error) => return Err(format!("读取 settings.json 失败: {error}")),
        };
        if !settings.is_object() {
            return Err("settings.json 必须是 JSON 对象".into());
        }
        let download_root = match std::env::var_os("HOTDOWNLOADER_DOWNLOAD_DIR") {
            Some(directory) => PathBuf::from(directory),
            None => data_dir.join("downloads"),
        };
        if !download_root.is_absolute() {
            return Err("独立服务下载目录必须是绝对路径".into());
        }
        std::fs::create_dir_all(&download_root)
            .map_err(|error| format!("创建下载目录失败: {error}"))?;
        let default_download_dir = download_root.to_string_lossy().to_string();
        let mut settings = settings;
        // 网页不能选择容器外任意路径，实际下载目录只由服务启动环境决定。
        settings["downloadDir"] = Value::String(default_download_dir.clone());
        if let Some(object) = settings.as_object_mut() {
            object.remove("safFolderUri");
            object.remove("safFolderName");
        }
        Ok(Self {
            settings: RwLock::new(settings),
            settings_path: path,
            default_download_dir,
        })
    }

    pub fn settings_snapshot(&self) -> SettingsSnapshot {
        snapshot(&self.settings.read().unwrap())
    }

    pub fn default_download_dir(&self) -> &str {
        &self.default_download_dir
    }

    pub fn artist_separator(&self) -> String {
        self.settings.read().unwrap()["artistSeparator"]
            .as_str()
            .filter(|value| !value.is_empty())
            .unwrap_or("、")
            .to_string()
    }

    pub fn patch_settings(
        &self,
        patch: SettingsPatch,
    ) -> Result<(SettingsSnapshot, bool), SettingsPatchError> {
        // 写锁覆盖比较原值、合并、写盘和内存更新。两个请求即使同时到达，
        // 后一个也只能基于前一个已提交的状态判断冲突。
        let mut current = self.settings.write().unwrap();
        let applied = apply_patch(&current, patch, SettingsScope::Web)?;
        if applied.changed_fields.is_empty() {
            return Ok((applied.snapshot, false));
        }
        let concurrency_changed = applied
            .changed_fields
            .iter()
            .any(|field| field == "maxConcurrent");
        let bytes = serde_json::to_vec_pretty(&applied.stored)
            .map_err(|error| SettingsPatchError::Invalid(error.to_string()))?;
        let temporary = self.settings_path.with_extension("json.tmp");
        std::fs::write(&temporary, bytes).map_err(|error| {
            SettingsPatchError::Invalid(format!("写入设置临时文件失败: {error}"))
        })?;
        #[cfg(windows)]
        if self.settings_path.exists() {
            std::fs::copy(&temporary, &self.settings_path).map_err(|error| {
                SettingsPatchError::Invalid(format!("更新设置文件失败: {error}"))
            })?;
            let _ = std::fs::remove_file(&temporary);
        } else {
            std::fs::rename(&temporary, &self.settings_path).map_err(|error| {
                SettingsPatchError::Invalid(format!("保存设置文件失败: {error}"))
            })?;
        }
        #[cfg(not(windows))]
        std::fs::rename(&temporary, &self.settings_path)
            .map_err(|error| SettingsPatchError::Invalid(format!("保存设置文件失败: {error}")))?;

        *current = applied.stored;
        Ok((applied.snapshot, concurrency_changed))
    }

    pub fn max_concurrent(&self) -> u32 {
        self.settings.read().unwrap()["maxConcurrent"]
            .as_u64()
            .map(|value| value.clamp(1, 32) as u32)
            .unwrap_or(3)
    }
}

impl DownloadConfigProvider for ServerEnvironment {
    fn current(&self) -> DownloadConfig {
        DownloadConfig::from_settings(&self.settings.read().unwrap(), &self.default_download_dir)
    }
}

impl TaskEnvironment for ServerEnvironment {
    fn task_rules(&self) -> TaskRules {
        TaskRules::from_settings(&self.settings.read().unwrap())
    }

    fn download_config(&self) -> DownloadConfig {
        self.current()
    }

    fn file_exists(&self, path: &str, is_saf: bool, _saf_folder_uri: Option<&str>) -> bool {
        !is_saf && Path::new(path).exists()
    }

    fn file_len(&self, path: &str, is_saf: bool) -> Option<u64> {
        if is_saf {
            None
        } else {
            std::fs::metadata(path).ok().map(|metadata| metadata.len())
        }
    }
}

/// 进度先更新 TaskState，再由 TaskState 的事件端口广播完整任务记录。
struct ServerProgress {
    tasks: Arc<TaskState>,
    events: Arc<ServerEvents>,
}

impl DownloadProgressSink for ServerProgress {
    fn progress(&self, task_id: &str, downloaded: u64, total: u64, speed: u64) {
        self.tasks.progress(task_id, downloaded, total, speed);
    }

    fn file_complete(&self, task_id: &str) {
        self.tasks.file_complete(task_id);
    }

    fn completed(&self, task_id: &str, final_path: &str, _saf_folder_uri: Option<String>) {
        self.tasks.completed(task_id, final_path);
    }

    fn error(&self, task_id: &str, message: &str) {
        self.tasks.failed(task_id, message, None);
    }

    fn link_expired(&self, task_id: &str, current_offset: u64) {
        self.tasks.failed(task_id, "链接过期", Some(current_offset));
    }

    fn metadata_error(&self, task_id: &str, message: &str) {
        self.events.send(
            "download-metadata-error",
            serde_json::json!({ "taskId": task_id, "errorMsg": message }).to_string(),
        );
    }
}

struct ServerRunner {
    settings: Arc<ServerEnvironment>,
    links: Arc<PlatformDownloadLinkProvider>,
    progress: Arc<ServerProgress>,
}

impl DownloadTaskRunner for ServerRunner {
    fn run(&self, context: TaskContext, controller: TaskController) -> BoxFuture<'static, bool> {
        let config = self.settings.current();
        let links = self.links.clone();
        let progress = self.progress.clone();
        Box::pin(async move {
            let opener = LocalDownloadFileOpener;
            let deleter = LocalFileDeleter;
            let postprocessor = LocalDownloadPostprocessor;
            download_task(
                context,
                controller,
                config,
                links.as_ref(),
                progress.as_ref(),
                &opener,
                &deleter,
                &postprocessor,
            )
            .await
        })
    }

    fn report_error(&self, task_id: &str, message: &str) {
        self.progress.error(task_id, message);
    }
}

/// 进程级状态。浏览器关闭只会丢弃 HTTP/SSE 连接，不会丢弃这里的引擎。
pub struct ServerRuntime {
    pub tasks: Arc<TaskState>,
    pub engine: DownloadEngine,
    pub environment: Arc<ServerEnvironment>,
    pub events: Arc<ServerEvents>,
    pub login_store: Arc<FileLoginStore>,
    api_token: Option<String>,
    pub web_dir: PathBuf,
    settings_patch_lock: Mutex<()>,
}

impl ServerRuntime {
    pub fn start(data_dir: PathBuf) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(&data_dir).map_err(|error| format!("创建数据目录失败: {error}"))?;
        let data_dir = data_dir
            .canonicalize()
            .map_err(|error| format!("解析数据目录失败: {error}"))?;
        let environment = Arc::new(ServerEnvironment::load(&data_dir)?);
        let events = Arc::new(ServerEvents::new());
        let repository = Arc::new(JsonTaskRepository::new(data_dir.join("tasks.json")));
        let tasks = Arc::new(TaskState::load(repository, events.clone())?);
        let progress = Arc::new(ServerProgress {
            tasks: tasks.clone(),
            events: events.clone(),
        });
        let links = Arc::new(PlatformDownloadLinkProvider::from_credentials_file(
            data_dir.join("qq-credentials.json"),
        ));
        let login_store = Arc::new(FileLoginStore::new(data_dir.join("qq-credentials.json")));
        let runner = Arc::new(ServerRunner {
            settings: environment.clone(),
            links,
            progress,
        });
        let engine = DownloadEngine::new(
            runner,
            Arc::new(LocalFileDeleter),
            Arc::new(NoopCompletionNotifier),
        );
        engine.set_concurrency(environment.max_concurrent());
        let runtime = Arc::new(Self {
            tasks,
            engine: engine.clone(),
            environment,
            events,
            login_store,
            settings_patch_lock: Mutex::new(()),
            api_token: std::env::var("HOTDOWNLOADER_TOKEN")
                .ok()
                .filter(|value| !value.is_empty()),
            web_dir: std::env::var_os("HOTDOWNLOADER_WEB_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("./dist")),
        });
        tokio::spawn(async move {
            // 每个下载任务在引擎内独立结束；一个任务的失败不会终止调度循环。
            engine.run_scheduler().await;
        });
        Ok(runtime)
    }

    pub fn patch_settings(
        &self,
        patch: SettingsPatch,
    ) -> Result<SettingsSnapshot, SettingsPatchError> {
        // 事件与调度器更新也按提交顺序执行，订阅者不会先收到较新的修订。
        let _guard = self.settings_patch_lock.lock().unwrap();
        let before = self.environment.settings_snapshot().revision;
        let (saved, concurrency_changed) = self.environment.patch_settings(patch)?;
        if concurrency_changed {
            self.engine
                .set_concurrency(self.environment.max_concurrent());
        }
        if saved.revision != before {
            self.events
                .send("settings-updated", serde_json::to_string(&saved).unwrap());
        }
        Ok(saved)
    }

    pub fn accepts_token(&self, supplied: Option<&str>) -> bool {
        let Some(expected) = self.api_token.as_deref() else {
            return true;
        };
        let Some(supplied) = supplied else {
            return false;
        };
        // 令牌比较不因首个不匹配字节提前返回，避免暴露前缀匹配长度。
        if expected.len() != supplied.len() {
            return false;
        }
        expected
            .bytes()
            .zip(supplied.bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    use serde_json::{json, Map};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn concurrent_field_updates_are_merged_and_persisted() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("hotdownloader-settings-{nonce}"));
        std::fs::create_dir_all(&directory).unwrap();
        let environment = Arc::new(ServerEnvironment::load(&directory).unwrap());

        // 两个页面都从旧快照出发，但修改不同字段；写锁应让两次持久化都保留。
        let handles: Vec<_> = [
            ("maxConcurrent", json!(5)),
            ("namingTemplate", json!("{artist}")),
        ]
        .into_iter()
        .map(|(field, value)| {
            let environment = environment.clone();
            std::thread::spawn(move || {
                environment
                    .patch_settings(SettingsPatch {
                        changes: Map::from_iter([(field.to_string(), value)]),
                        expected: Map::from_iter([(field.to_string(), Value::Null)]),
                    })
                    .unwrap();
            })
        })
        .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let saved: Value =
            serde_json::from_slice(&std::fs::read(directory.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(saved["maxConcurrent"], 5);
        assert_eq!(saved["namingTemplate"], "{artist}");
        assert_eq!(saved["_settingsRevision"], 2);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

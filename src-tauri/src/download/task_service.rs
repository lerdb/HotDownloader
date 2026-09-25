//! Tauri 任务服务适配层。业务方法位于共享核心，这里只连接设置存储、SAF 和托管状态。

use hotdownloader_core::download_config::{DownloadConfig, DownloadConfigProvider};
use hotdownloader_core::task_rules::TaskRules;
use hotdownloader_core::task_service::{TaskEnvironment, TaskService};
use tauri::{AppHandle, Manager};
use tauri_plugin_android_fs::{AndroidFsExt, FsUri};

use crate::adapters::tauri_download_host::TauriDownloadConfigProvider;
use crate::download::contract::{BatchResult, CreateTaskRequest, CreateTaskResult, TaskRecord};
use crate::download::engine::DownloadEngine;
use crate::download::task_state::TaskState;
use crate::storage::store_wrapper;

/// 创建、重试时读取当下设置，避免页面持有过期的重试和降级策略。
/// 本地文件长度直接从磁盘读取；SAF 文件名必须由 Android 插件在授权目录内解析。
struct TauriTaskEnvironment {
    app: AppHandle,
}

impl TauriTaskEnvironment {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl TaskEnvironment for TauriTaskEnvironment {
    fn task_rules(&self) -> TaskRules {
        let raw = store_wrapper::load_string(&self.app, "settings").unwrap_or_default();
        let settings = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
        TaskRules::from_settings(&settings)
    }

    fn download_config(&self) -> DownloadConfig {
        TauriDownloadConfigProvider::new(self.app.clone()).current()
    }

    fn file_exists(&self, path: &str, is_saf: bool, saf_folder_uri: Option<&str>) -> bool {
        if !is_saf {
            return std::path::Path::new(path).exists();
        }

        // 核心只传文件名和设置快照中的目录 URI；Android 插件负责访问授权目录。
        let Some(parent_uri) = saf_folder_uri.and_then(|uri| FsUri::from_json_str(uri).ok()) else {
            return false;
        };
        self.app
            .android_fs()
            .resolve_file_uri(&parent_uri, std::path::Path::new(path))
            .is_ok()
    }

    fn file_len(&self, path: &str, is_saf: bool) -> Option<u64> {
        if is_saf {
            // std::fs 无法读取 SAF 内容；续传策略保持由 worker 从头处理。
            return None;
        }
        std::fs::metadata(path).ok().map(|metadata| metadata.len())
    }
}

// 每个 IPC 命令只创建轻量借用对象。TaskState 和 DownloadEngine 仍由应用进程持有，
// 页面离开不会使正在执行的任务或调度器退出。
pub async fn load_tasks(app: AppHandle) -> Result<Vec<TaskRecord>, String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    Ok(TaskService::new(&state, &engine, &environment).load_tasks())
}

pub async fn create_download_task(
    app: AppHandle,
    request: CreateTaskRequest,
) -> Result<CreateTaskResult, String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    TaskService::new(&state, &engine, &environment)
        .create_download_task(request)
        .await
}

pub async fn pause_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    TaskService::new(&state, &engine, &environment)
        .pause_task(task_id)
        .await
}

pub async fn resume_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    TaskService::new(&state, &engine, &environment)
        .resume_task(task_id)
        .await
}

pub async fn cancel_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    TaskService::new(&state, &engine, &environment)
        .cancel_task(task_id, delete_file)
        .await
}

pub async fn remove_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    cancel_task(app, task_id, delete_file).await
}

/// 批量删除在同一个服务实例中串行执行，保留每个失败任务的具体错误。
pub async fn remove_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
    delete_file: bool,
) -> Result<BatchResult, String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    TaskService::new(&state, &engine, &environment)
        .remove_tasks(task_ids, delete_file)
        .await
}

pub async fn retry_task(app: AppHandle, task_id: String) -> Result<bool, String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    TaskService::new(&state, &engine, &environment)
        .retry_task(task_id)
        .await
}

pub fn set_max_concurrent(app: AppHandle, max: u32) -> Result<(), String> {
    let state = app.state::<TaskState>();
    let engine = app.state::<DownloadEngine>();
    let environment = TauriTaskEnvironment::new(app.clone());
    TaskService::new(&state, &engine, &environment).set_max_concurrent(max)
}

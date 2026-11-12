//! Tauri 任务命令适配层：只负责暴露 IPC 入口，业务规则位于 task_service。

use tauri::{command, AppHandle};

use crate::storage::{recovery::RecoveryReport, store_wrapper};

use crate::download::task_service;
use hotdownloader_core::task::contract::{
    BatchResult, CreateTaskRequest, CreateTaskResult, TaskRecord,
};

// store_wrapper 的整表写盘可能较慢，命令保持 async，避免阻塞 Tauri 主线程。
#[command]
pub async fn load_tasks(app: AppHandle) -> Result<Vec<TaskRecord>, String> {
    task_service::load_tasks(app).await
}

// 启动时的事件可能早于前端监听，因此由挂载后的页面主动读取恢复报告。
#[command]
pub fn get_storage_recovery_report(app: AppHandle) -> Option<RecoveryReport> {
    store_wrapper::recovery_report(&app)
}

#[command]
pub async fn create_download_task(
    app: AppHandle,
    request: CreateTaskRequest,
) -> Result<CreateTaskResult, String> {
    task_service::create_download_task(app, request).await
}

#[command]
pub async fn pause_task(app: AppHandle, task_id: String) -> Result<(), String> {
    task_service::pause_task(app, task_id).await
}

#[command]
pub async fn resume_task(app: AppHandle, task_id: String) -> Result<(), String> {
    task_service::resume_task(app, task_id).await
}

#[command]
pub async fn cancel_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    task_service::cancel_task(app, task_id, delete_file).await
}

#[command]
pub async fn remove_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    task_service::remove_task(app, task_id, delete_file).await
}

#[command]
pub async fn remove_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
    delete_file: bool,
) -> Result<BatchResult, String> {
    task_service::remove_tasks(app, task_ids, delete_file).await
}

#[command]
pub async fn retry_task(
    app: AppHandle,
    task_id: String,
    quality: Option<String>,
) -> Result<bool, String> {
    task_service::retry_task(app, task_id, quality).await
}

#[command]
pub fn queue_status(app: AppHandle) -> hotdownloader_core::download::queue::QueueStatus {
    task_service::queue_status(app)
}

#[command]
pub async fn control_queue(app: AppHandle, action: String) -> Result<BatchResult, String> {
    task_service::control_queue(app, action).await
}

#[command]
pub async fn control_tasks(
    app: AppHandle,
    request: hotdownloader_core::task::contract::BatchControlRequest,
) -> Result<BatchResult, String> {
    task_service::control_tasks(app, request).await
}

#[command]
pub fn set_max_concurrent(app: AppHandle, max: u32) -> Result<(), String> {
    task_service::set_max_concurrent(app, max)
}

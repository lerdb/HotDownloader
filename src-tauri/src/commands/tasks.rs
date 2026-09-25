//! Tauri 任务命令适配层：只负责暴露 IPC 入口，业务规则位于 task_service。

use tauri::{command, AppHandle};

use crate::download::contract::{BatchResult, CreateTaskRequest, CreateTaskResult, TaskRecord};
use crate::download::task_service;

// store_wrapper 的整表写盘可能较慢，命令保持 async，避免阻塞 Tauri 主线程。
#[command]
pub async fn load_tasks(app: AppHandle) -> Result<Vec<TaskRecord>, String> {
    task_service::load_tasks(app).await
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
pub async fn retry_task(app: AppHandle, task_id: String) -> Result<bool, String> {
    task_service::retry_task(app, task_id).await
}

#[command]
pub fn set_max_concurrent(app: AppHandle, max: u32) -> Result<(), String> {
    task_service::set_max_concurrent(app, max)
}

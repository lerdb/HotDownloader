use super::task_state::TaskState;
use crate::events;
use tauri::{AppHandle, Emitter, Manager};

pub fn emit_progress(
    app_handle: &AppHandle,
    task_id: &str,
    downloaded: u64,
    total: u64,
    speed: u64,
) {
    // 先更新 Rust 持有的任务快照，再发兼容事件供其他 UI 功能使用。
    app_handle
        .state::<TaskState>()
        .progress(task_id, downloaded, total, speed);
    let payload = events::DownloadProgressPayload {
        task_id: task_id.to_string(),
        downloaded,
        total,
        speed,
    };
    let _ = app_handle.emit(events::DOWNLOAD_PROGRESS, payload);
}

pub fn emit_completed(
    app_handle: &AppHandle,
    task_id: &str,
    final_path: &str,
    saf_folder_uri: Option<String>,
) {
    // 完成状态由后端落盘；页面关闭不影响任务结果。
    app_handle
        .state::<TaskState>()
        .completed(task_id, final_path);
    let payload = events::DownloadCompletedPayload {
        task_id: task_id.to_string(),
        final_path: final_path.to_string(),
        saf_folder_uri,
    };
    let _ = app_handle.emit(events::DOWNLOAD_COMPLETED, payload);
}

pub fn emit_error(app_handle: &AppHandle, task_id: &str, error_msg: &str) {
    // 失败原因先写入任务记录，前端只负责展示。
    app_handle
        .state::<TaskState>()
        .failed(task_id, error_msg, None);
    let payload = events::DownloadErrorPayload {
        task_id: task_id.to_string(),
        error_msg: error_msg.to_string(),
    };
    let _ = app_handle.emit(events::DOWNLOAD_ERROR, payload);
}

pub fn emit_link_expired(app_handle: &AppHandle, task_id: &str, current_offset: u64) {
    // 保存当前偏移供任务列表展示；再次重试时仍会核对实际文件长度。
    app_handle
        .state::<TaskState>()
        .failed(task_id, "链接过期", Some(current_offset));
    let payload = events::DownloadLinkExpiredPayload {
        task_id: task_id.to_string(),
        current_offset,
    };
    let _ = app_handle.emit(events::DOWNLOAD_LINK_EXPIRED, payload);
}

/// 文件传输完成后先在 Rust 侧进入 processing，元数据处理完成才进入 completed。
pub fn emit_file_complete(app_handle: &AppHandle, task_id: &str) {
    app_handle.state::<TaskState>().file_complete(task_id);
    let payload = events::DownloadFileCompletePayload {
        task_id: task_id.to_string(),
    };
    let _ = app_handle.emit(events::DOWNLOAD_FILE_COMPLETE, payload);
}

/// 发送元数据写入失败事件，前端弹出警告提示
pub fn emit_metadata_error(app_handle: &AppHandle, task_id: &str, error_msg: &str) {
    let payload = events::DownloadMetadataErrorPayload {
        task_id: task_id.to_string(),
        error_msg: error_msg.to_string(),
    };
    let _ = app_handle.emit(events::DOWNLOAD_METADATA_ERROR, payload);
}

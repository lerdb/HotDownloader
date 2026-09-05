use serde::Serialize;

pub const DOWNLOAD_PROGRESS: &str = "download-progress";
pub const DOWNLOAD_COMPLETED: &str = "download-completed";
pub const DOWNLOAD_ERROR: &str = "download-error";
pub const DOWNLOAD_LINK_EXPIRED: &str = "download-link-expired";
// 文件下载完成，正在处理元数据/歌词
pub const DOWNLOAD_FILE_COMPLETE: &str = "download-file-complete";
// 元数据写入失败（仅提示，不阻断完成）
pub const DOWNLOAD_METADATA_ERROR: &str = "download-metadata-error";
// 登录凭证刷新失败（自动刷新场景，用于通知前端弹窗提示用户）
pub const LOGIN_REFRESH_FAILED: &str = "login-refresh-failed";

#[derive(Serialize, Clone)]
pub struct DownloadProgressPayload {
    pub task_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
}

#[derive(Serialize, Clone)]
pub struct DownloadCompletedPayload {
    pub task_id: String,
    pub final_path: String,
    pub saf_folder_uri: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct DownloadErrorPayload {
    pub task_id: String,
    pub error_msg: String,
}

#[derive(Serialize, Clone)]
pub struct DownloadLinkExpiredPayload {
    pub task_id: String,
    pub current_offset: u64,
}

// 文件下载完成，处理中事件载荷
#[derive(Serialize, Clone)]
pub struct DownloadFileCompletePayload {
    pub task_id: String,
}

// 元数据写入失败事件载荷
#[derive(Serialize, Clone)]
pub struct DownloadMetadataErrorPayload {
    pub task_id: String,
    pub error_msg: String,
}

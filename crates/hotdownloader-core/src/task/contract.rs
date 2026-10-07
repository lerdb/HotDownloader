use serde::{Deserialize, Serialize};

/// 歌曲的可下载品质；文件名和大小与品质绑定，降级时需要一并更新。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityItem {
    pub quality: String,
    pub filename: String,
    pub size: u64,
}

/// 前端提交的歌曲元数据。具体下载行为由 Rust 侧根据设置和任务规则决定。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongInput {
    pub platform: String,
    pub id: u64,
    pub mid: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub media_mid: String,
    pub qualities: Vec<QualityItem>,
}

// 引入平台字段前仅支持 QQ 音乐，历史任务缺失该字段时沿用原平台。
fn legacy_task_platform() -> String {
    "qqmusic".to_owned()
}

/// 持久化任务契约：字段名沿用前端已有的 camelCase 数据，兼容旧任务记录。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    #[serde(default = "legacy_task_platform")]
    pub platform: String,
    pub song_id: u64,
    pub song_mid: String,
    pub song_title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub media_mid: String,
    pub filename: String,
    pub quality: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    pub file_size: u64,
    pub downloaded: u64,
    pub retry_count: u32,
    pub added_at: u64,
    // 旧记录可能没有品质列表；这类任务重试时无法自动选择降级品质。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_qualities: Option<Vec<QualityItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<u64>,
    // 目标路径在建任务时确定，保证排队任务也能参与重名冲突检查。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Waiting,
    Downloading,
    Paused,
    Processing,
    Completed,
    Error,
    /// 进程重启后等待用户恢复的任务；恢复不消耗下载错误重试次数。
    Interrupted,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateAction {
    Overwrite,
    Rename,
    Cancel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub song: SongInput,
    pub desired_quality: String,
    pub duplicate_action: Option<DuplicateAction>,
}

/// ask 策略需要用户选择时返回 NeedsConfirmation；任务尚未写入持久化列表。
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CreateTaskResult {
    Created { task: Box<TaskRecord> },
    Existing { task: Box<TaskRecord> },
    NeedsConfirmation { song_title: String },
    Cancelled,
}

#[derive(Debug, Serialize)]
pub struct BatchResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_existing_frontend_task_record() {
        let old_record = serde_json::json!({
            "id": "old-1", "songId": 42, "songMid": "mid",
            "songTitle": "Title", "artist": "Artist", "album": "Album",
            "filename": "song.mp3", "quality": "320kmp3", "status": "error",
            "fileSize": 100, "downloaded": 0, "retryCount": 1, "addedAt": 1234
        });
        let task: TaskRecord = serde_json::from_value(old_record).unwrap();
        assert_eq!(task.platform, "qqmusic");
        assert_eq!(task.song_id, 42);
        assert!(task.cover_url.is_empty());
        assert!(task.media_mid.is_empty());
        assert!(task.available_qualities.is_none());
        assert!(task.save_path.is_none());
        let serialized = serde_json::to_value(&task).unwrap();
        assert_eq!(serialized["platform"], "qqmusic");
        assert_eq!(serialized["songTitle"], "Title");
        assert_eq!(serialized["status"], "error");

        // Box 只改变 Rust 内存布局，不改变 IPC 和 HTTP 共用的响应 JSON。
        let result = serde_json::to_value(CreateTaskResult::Created {
            task: Box::new(task),
        })
        .unwrap();
        assert_eq!(result["outcome"], "created");
        assert_eq!(result["task"], serialized);
    }

    #[test]
    fn loads_mixed_task_history_and_preserves_existing_platforms() {
        let record = serde_json::json!({
            "id": "legacy", "songId": 42, "songMid": "mid",
            "songTitle": "Title", "artist": "Artist", "album": "Album",
            "filename": "song.mp3", "quality": "320kmp3", "status": "completed",
            "filePath": "D:/Music/song.mp3", "fileSize": 100, "downloaded": 100,
            "retryCount": 0, "addedAt": 1234
        });
        let mut qqmusic = record.clone();
        qqmusic["id"] = serde_json::json!("qq");
        qqmusic["platform"] = serde_json::json!("qqmusic");
        let mut kuwo = record.clone();
        kuwo["id"] = serde_json::json!("kw");
        kuwo["platform"] = serde_json::json!("kuwo");
        let json = serde_json::json!([record, qqmusic, kuwo]).to_string();

        let tasks: Vec<TaskRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].platform, "qqmusic");
        assert_eq!(tasks[1].platform, "qqmusic");
        assert_eq!(tasks[2].platform, "kuwo");
        assert_eq!(tasks[0].status, TaskStatus::Completed);
        assert_eq!(tasks[0].file_path.as_deref(), Some("D:/Music/song.mp3"));
        assert_eq!(tasks[0].downloaded, 100);

        let saved = serde_json::to_string(&tasks).unwrap();
        let reloaded: Vec<TaskRecord> = serde_json::from_str(&saved).unwrap();
        assert_eq!(reloaded[0].platform, "qqmusic");
        assert_eq!(reloaded[2].platform, "kuwo");
    }
}

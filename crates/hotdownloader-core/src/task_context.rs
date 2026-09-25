use std::sync::Arc;

use tokio::sync::Mutex;

use super::platform::Platform;

/// 歌曲信息，用于文件命名及元数据写入。
#[derive(Clone)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub quality: String,
    pub cover_url: String,
}

/// 调度器传给下载 worker 的完整上下文，不包含任何 Tauri 句柄。
#[derive(Clone)]
pub struct TaskContext {
    pub task_id: String,
    pub platform: Platform,
    /// 歌曲字符串标识，QQ 音乐使用 mid。
    pub song_mid: String,
    /// 歌曲数字 ID，供平台接口使用。
    pub song_id: u64,
    pub url: String,
    /// 创建任务时确定的目标位置；普通路径为绝对路径，SAF 为文件名。
    pub save_path: String,
    #[allow(dead_code)] // 保留历史上下文字段，供后续链接策略使用。
    pub quality: String,
    #[allow(dead_code)]
    pub key: String,
    pub file_size: u64,
    pub downloaded_offset: u64,
    pub song_info: SongInfo,
    /// 平台提供的真实品质文件名，降级时与品质和大小一起更新。
    pub quality_filename: String,
    /// worker 确定的最终文件路径，与控制器共享，供任务删除时使用。
    pub final_path: Arc<Mutex<Option<String>>>,
}

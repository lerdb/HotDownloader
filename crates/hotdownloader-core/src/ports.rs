use std::fs::File;
use std::io::BufWriter;

use futures_util::future::BoxFuture;

use crate::download_config::DownloadConfig;
use crate::task_context::TaskContext;

/// 删除普通文件或平台专有 URI。下载引擎只知道目标路径，不解析 SAF。
pub trait FileDeleter: Send + Sync {
    fn remove<'a>(&'a self, path: &'a str) -> BoxFuture<'a, Result<(), String>>;
}

/// 下载完成后的可选外部提示。通知失败不得改变任务完成状态。
pub trait CompletionNotifier: Send + Sync {
    fn completed(&self, song_title: &str, artist: &str);
}

/// 下载 worker 的状态输出端口。状态变更应先更新权威任务记录，再广播给订阅者；
/// 元数据警告仅供提示，不改变已完成的任务状态。事件名称和传输协议由运行时决定。
pub trait DownloadProgressSink: Send + Sync {
    fn progress(&self, task_id: &str, downloaded: u64, total: u64, speed: u64);
    fn file_complete(&self, task_id: &str);
    fn completed(&self, task_id: &str, final_path: &str, saf_folder_uri: Option<String>);
    fn error(&self, task_id: &str, message: &str);
    fn link_expired(&self, task_id: &str, current_offset: u64);
    fn metadata_error(&self, task_id: &str, message: &str);
}

/// 打开下载文件所需的路径与偏移。普通路径为完整文件路径，SAF 模式为文件名和目录 URI。
pub struct FileOpenRequest<'a> {
    pub task_id: &'a str,
    pub file_path: &'a str,
    pub is_saf: bool,
    pub saf_folder_uri: Option<&'a str>,
    pub downloaded: u64,
}

/// 文件适配器返回最终偏移和 URI；文件异常时可能重置偏移，因此不能只返回写入器。
pub struct OpenedDownloadFile {
    pub writer: BufWriter<File>,
    pub downloaded: u64,
    pub saf_file_uri: Option<String>,
}

/// 平台文件打开端口。SAF 插件仅出现在 Tauri 实现中，普通文件可由核心实现。
pub trait DownloadFileOpener: Send + Sync {
    fn open<'a>(
        &'a self,
        request: FileOpenRequest<'a>,
        progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Option<OpenedDownloadFile>>;
}

/// 文件流完成后交给运行时处理歌词、封面和音频标签，返回可选的独立 LRC 路径。
/// 元数据失败不会使音频下载失败，具体警告由 `DownloadProgressSink` 输出。
pub struct PostprocessRequest<'a> {
    pub context: &'a TaskContext,
    pub audio_path: &'a str,
    pub is_saf: bool,
    pub saf_file_uri: Option<&'a str>,
    pub saf_folder_uri: Option<&'a str>,
    pub config: &'a DownloadConfig,
}

pub trait DownloadPostprocessor: Send + Sync {
    fn process<'a>(
        &'a self,
        request: PostprocessRequest<'a>,
        progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Option<String>>;
}

/// 不需要歌词或标签处理的独立运行时可先使用此实现。
pub struct NoopDownloadPostprocessor;

impl DownloadPostprocessor for NoopDownloadPostprocessor {
    fn process<'a>(
        &'a self,
        _request: PostprocessRequest<'a>,
        _progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Option<String>> {
        Box::pin(async { None })
    }
}

/// 无系统通知的运行时可使用此实现。
pub struct NoopCompletionNotifier;

impl CompletionNotifier for NoopCompletionNotifier {
    fn completed(&self, _song_title: &str, _artist: &str) {}
}

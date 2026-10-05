use futures_util::future::BoxFuture;
use hotdownloader_core::download::config::{DownloadConfig, DownloadConfigProvider};
use hotdownloader_core::download::link::{DownloadLinkProvider, PlatformDownloadLinkProvider};
use hotdownloader_core::platforms::qqmusic::credentials::{QqAuth, QqCredentialSource};
use hotdownloader_core::platforms::Platform;
use std::sync::Arc;
use tauri::Emitter;
use tauri::{AppHandle, Manager};
use tauri_plugin_android_fs::{AndroidFsExt, FsUri};

use crate::download::progress;
use crate::download::task_file::open_download_file;
use crate::storage::store_wrapper;
use hotdownloader_core::adapters::local::file_deleter::LocalFileDeleter;
use hotdownloader_core::download::context::TaskContext;
use hotdownloader_core::download::engine::{DownloadTaskRunner, TaskController};
use hotdownloader_core::download::ports::{
    CompletionNotifier, DownloadFileOpener, DownloadProgressSink, DownloadWorkerPorts, FileDeleter,
    FileOpenRequest, OpenedDownloadFile,
};
use hotdownloader_core::download::worker::download_task;

use super::tauri_postprocess::TauriDownloadPostprocessor;

/// 将调度器任务交给共享核心 worker，并组装桌面与 Android 所需的平台端口。
pub struct TauriTaskRunner {
    app: AppHandle,
}

impl TauriTaskRunner {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DownloadTaskRunner for TauriTaskRunner {
    fn run(&self, context: TaskContext, controller: TaskController) -> BoxFuture<'static, bool> {
        let app = self.app.clone();
        Box::pin(async move {
            // 每次任务启动时读取一次设置；worker 只接收快照，不直接访问 Tauri Store。
            let config = TauriDownloadConfigProvider::new(app.clone()).current();
            let link_provider = TauriDownloadLinkProvider::new(app.clone());
            let progress_sink = TauriDownloadProgressSink::new(app.clone());
            let file_deleter = TauriFileDeleter::new(app.clone());
            let file_opener = TauriDownloadFileOpener::new(app.clone());
            let postprocessor = TauriDownloadPostprocessor::new(app.clone());
            let tasks = app.state::<hotdownloader_core::task::state::TaskState>();
            let environment = crate::download::task_service::TauriTaskEnvironment::new(app.clone());
            let fallback = hotdownloader_core::download::fallback::TaskQualityFallback::new(
                &tasks,
                &environment,
            );
            download_task(
                context,
                controller,
                config,
                DownloadWorkerPorts {
                    quality_fallback: Some(&fallback),
                    link_provider: &link_provider,
                    progress_sink: &progress_sink,
                    file_opener: &file_opener,
                    file_deleter: &file_deleter,
                    postprocessor: &postprocessor,
                },
            )
            .await
        })
    }

    fn report_error(&self, task_id: &str, message: &str) {
        progress::emit_error(&self.app, task_id, message);
    }
}

/// 文件打开适配器：普通文件复用核心规则，Android SAF 由插件处理并返回真实 URI。
pub struct TauriDownloadFileOpener {
    app: AppHandle,
}

impl TauriDownloadFileOpener {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DownloadFileOpener for TauriDownloadFileOpener {
    fn prepare_parent(&self, file_path: &str) -> std::io::Result<()> {
        // 普通路径沿用共享文件规则；SAF 路径由 worker 在调用前排除。
        hotdownloader_core::adapters::local::download_file::ensure_parent_directory(file_path)
    }

    fn open<'a>(
        &'a self,
        request: FileOpenRequest<'a>,
        progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Option<OpenedDownloadFile>> {
        Box::pin(async move { open_download_file(&self.app, request, progress_sink).await })
    }
}

/// 将 worker 的状态输出接到现有 Rust 任务表和兼容 Tauri 事件。
pub struct TauriDownloadProgressSink {
    app: AppHandle,
}

impl TauriDownloadProgressSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DownloadProgressSink for TauriDownloadProgressSink {
    fn progress(&self, task_id: &str, downloaded: u64, total: u64, speed: u64) {
        progress::emit_progress(&self.app, task_id, downloaded, total, speed);
    }

    fn file_complete(&self, task_id: &str) {
        progress::emit_file_complete(&self.app, task_id);
    }

    fn completed(&self, task_id: &str, final_path: &str, saf_folder_uri: Option<String>) {
        progress::emit_completed(&self.app, task_id, final_path, saf_folder_uri);
    }

    fn error(&self, task_id: &str, message: &str) {
        progress::emit_error(&self.app, task_id, message);
    }

    fn link_expired(&self, task_id: &str, current_offset: u64) {
        progress::emit_link_expired(&self.app, task_id, current_offset);
    }

    fn metadata_error(&self, task_id: &str, message: &str) {
        progress::emit_metadata_error(&self.app, task_id, message);
    }
}

/// 桌面与 Android 的下载设置来源；默认目录沿用当前平台的路径选择逻辑。
pub struct TauriDownloadConfigProvider {
    app: AppHandle,
}

impl TauriDownloadConfigProvider {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DownloadConfigProvider for TauriDownloadConfigProvider {
    fn current(&self) -> DownloadConfig {
        let default_dir = crate::commands::file_ops::get_default_download_dir_impl(&self.app);
        let settings_json = store_wrapper::load_string(&self.app, "settings").unwrap_or_default();
        let settings = serde_json::from_str(&settings_json).unwrap_or(serde_json::json!({}));
        DownloadConfig::from_settings(&settings, &default_dir)
    }
}

/// 桌面端只提供当前 QQ 凭据；平台选择和链接 HTTP 请求统一位于共享核心。
pub struct TauriDownloadLinkProvider {
    provider: PlatformDownloadLinkProvider,
}

struct TauriQqCredentialSource {
    app: AppHandle,
}

impl TauriDownloadLinkProvider {
    pub fn new(app: AppHandle) -> Self {
        let credentials = Arc::new(TauriQqCredentialSource { app });
        Self {
            provider: PlatformDownloadLinkProvider::new(credentials),
        }
    }
}

impl QqCredentialSource for TauriQqCredentialSource {
    fn current(&self) -> BoxFuture<'_, Result<Option<QqAuth>, String>> {
        Box::pin(async move {
            let resolved = crate::platforms::qqmusic::login::download_auth(&self.app).await;
            if let Some(error) = resolved.refresh_error {
                // 核心保留旧凭据；Tauri 负责向当前窗口报告刷新失败。
                log::warn!("QQ音乐凭证刷新失败，继续使用旧凭证: {error}");
                let _ = self.app.emit(
                    crate::events::LOGIN_REFRESH_FAILED,
                    format!("QQ音乐登录已过期，自动刷新失败：{error}"),
                );
            }
            Ok(resolved.auth)
        })
    }
}

impl DownloadLinkProvider for TauriDownloadLinkProvider {
    fn fetch<'a>(
        &'a self,
        platform: Platform,
        song_mid: &'a str,
        filename: &'a str,
    ) -> BoxFuture<'a, Result<(String, String), String>> {
        self.provider.fetch(platform, song_mid, filename)
    }
}

/// Tauri 文件适配器：普通路径复用本地文件实现，SAF URI 交给 Android 插件。
pub struct TauriFileDeleter {
    app: AppHandle,
}

impl TauriFileDeleter {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl FileDeleter for TauriFileDeleter {
    fn remove<'a>(&'a self, path: &'a str) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if path.starts_with("content://") || path.starts_with("saf://") {
                let uri = FsUri::from_uri(path.to_string());
                self.app
                    .android_fs()
                    .remove_file(&uri)
                    .map_err(|error| format!("删除 SAF 文件失败: {error}"))
            } else {
                LocalFileDeleter.remove(path).await
            }
        })
    }
}

/// Tauri 通知适配器：设置读取和系统通知留在平台层。
pub struct TauriCompletionNotifier {
    app: AppHandle,
}

impl TauriCompletionNotifier {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl CompletionNotifier for TauriCompletionNotifier {
    fn completed(&self, song_title: &str, artist: &str) {
        let enabled = store_wrapper::load_string(&self.app, "settings")
            .ok()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .and_then(|settings| settings.get("notifyOnComplete")?.as_bool())
            .unwrap_or(false);

        if enabled {
            crate::commands::notify::send_download_complete_notification(
                &self.app, song_title, artist,
            );
        } else {
            log::info!("用户已关闭下载完成通知，跳过发送");
        }
    }
}

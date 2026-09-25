use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::atomic::Ordering;

use crate::decryption;
use crate::download_config::DownloadConfig;
use crate::download_link::{fetch_download_link_with_retry, DownloadLinkProvider};
use crate::download_path::resolve_download_path;
use crate::engine::TaskController;
use crate::http_transfer::{
    classify_http_response, request_download_response, write_response_stream, ResponseAction,
    StreamOutcome,
};
use crate::local_download_file::ensure_parent_directory;
use crate::ports::{
    DownloadFileOpener, DownloadPostprocessor, DownloadProgressSink, FileDeleter, FileOpenRequest,
    PostprocessRequest,
};
use crate::task_context::TaskContext;

/// 与 Tauri 无关的完整下载任务循环。运行时提供设置、链接、文件、状态和收尾端口。
/// 返回任务是否成功完成，供调度器决定是否清理任务上下文。
pub async fn download_task(
    ctx: TaskContext,
    controller: TaskController,
    config: DownloadConfig,
    link_provider: &dyn DownloadLinkProvider,
    progress_sink: &dyn DownloadProgressSink,
    file_opener: &dyn DownloadFileOpener,
    file_deleter: &dyn FileDeleter,
    postprocessor: &dyn DownloadPostprocessor,
) -> bool {
    // 1. 构建最终保存路径（只需一次）
    // 创建任务时已经确定目标路径；Android SAF 任务需保留 URI 上下文，
    // 否则重试时会把相对文件名当成本地文件系统路径。
    let (is_saf, download_dir, saf_folder_uri) = if !ctx.save_path.is_empty() {
        if config.download_dir == "saf://"
            && cfg!(target_os = "android")
            && config.saf_folder_uri.is_some()
        {
            (true, ctx.save_path.clone(), config.saf_folder_uri.clone())
        } else {
            (false, ctx.save_path.clone(), None)
        }
    } else {
        resolve_download_path(
            &config.download_dir,
            &config.naming_template,
            config.saf_folder_uri.as_deref(),
            &ctx.song_info,
            &ctx.quality_filename,
        )
    };

    log::info!("任务 {} 开始下载，文件路径: {}", ctx.task_id, download_dir);

    // 2. 创建目录并验证（仅普通模式需要）
    if !is_saf {
        if let Err(error) = ensure_parent_directory(&download_dir) {
            log::error!("创建下载目录失败: {}", error);
            progress_sink.error(&ctx.task_id, "下载目录无法访问");
            return false;
        }
    }

    // 3. 初始化已下载偏移量
    let mut downloaded = ctx.downloaded_offset;

    // 4. 链接与解密密钥（每次循环可能重新获取）
    let mut url = String::new();
    let mut key = String::new();

    // 5. 文件句柄（使用 BufWriter 提升写入性能）
    let mut file: Option<BufWriter<File>> = None;

    let mut saf_file_uri: Option<String> = None;

    // 流错误重试计数跨重新获取链接保留，防止持续断流造成无限重试。
    let mut stream_retries: u32 = 0;

    let mut completed_ok = false; // 标记下载是否真正完成

    // 下载循环
    'download: loop {
        // 检查取消
        if controller.cancel_token.is_cancelled() {
            break 'download;
        }

        // 初始暂停等待（使用统一的辅助函数）
        while controller.pause_flag.load(Ordering::SeqCst) {
            if controller.wait_for_resume_or_cancel().await {
                break 'download; // 任务被取消
            }
        }

        // 如果没有有效链接，实时获取（首次进入或暂停恢复后）
        if url.is_empty() {
            match fetch_download_link_with_retry(
                link_provider,
                &ctx.song_mid,
                &ctx.quality_filename,
                &ctx.task_id,
                ctx.platform, // 添加平台参数
            )
            .await
            {
                Ok((new_url, new_key)) => {
                    url = new_url;
                    key = new_key;
                    log::info!("任务 {} 获取到新下载链接", ctx.task_id);
                }
                Err(e) => {
                    // 将具体错误信息发送到前端，便于用户了解失败原因
                    log::error!("任务 {} 最终获取下载链接失败: {}", ctx.task_id, e);
                    progress_sink.error(&ctx.task_id, &format!("获取下载链接失败: {}", e));
                    break 'download;
                }
            }
        }

        // 文件名和 ekey 一并交给核心，续传时解密器仍按绝对字节偏移工作。
        let decrypt_ctx = decryption::init_for_filename(&ctx.quality_filename, &key);

        // 打开/续传文件
        if file.is_none() {
            let request = FileOpenRequest {
                task_id: &ctx.task_id,
                file_path: &download_dir,
                is_saf,
                saf_folder_uri: saf_folder_uri.as_deref(),
                downloaded,
            };
            match file_opener.open(request, progress_sink).await {
                Some(opened) => {
                    // 文件可能比持久化偏移短，适配器会清空文件并把偏移重置为零。
                    downloaded = opened.downloaded;
                    saf_file_uri = opened.saf_file_uri;
                    file = Some(opened.writer);

                    // 更新 final_path：SAF 模式为 URI，普通模式为普通路径
                    if is_saf {
                        if let Some(uri) = saf_file_uri.clone() {
                            *controller.final_path.lock().await = Some(uri);
                        }
                    } else {
                        *controller.final_path.lock().await = Some(download_dir.clone());
                    }
                }
                None => break 'download,
            }
        }

        // 请求重试、Range 头及响应校验均由核心处理；这里执行需要文件适配器的动作。
        let response = match request_download_response(&url, downloaded, &ctx.task_id).await {
            Ok(response) => response,
            Err(error) => {
                log::error!("任务 {} 最终下载请求失败: {}", ctx.task_id, error);
                progress_sink.error(&ctx.task_id, &format!("下载请求失败: {}", error));
                break 'download;
            }
        };

        let total = match classify_http_response(&response, downloaded, ctx.file_size) {
            ResponseAction::RestartFromBeginning { total } => {
                // 部分 CDN 忽略 Range 并返回完整文件。不能将完整响应追加到已有片段，
                // 因此重新打开目标文件并从 0 开始写入。
                log::warn!("任务 {} 的服务器忽略续传范围，改为从头下载", ctx.task_id);
                file.take();
                downloaded = 0;
                let request = FileOpenRequest {
                    task_id: &ctx.task_id,
                    file_path: &download_dir,
                    is_saf,
                    saf_folder_uri: saf_folder_uri.as_deref(),
                    downloaded,
                };
                let Some(opened) = file_opener.open(request, progress_sink).await else {
                    break 'download;
                };
                downloaded = opened.downloaded;
                saf_file_uri = opened.saf_file_uri;
                file = Some(opened.writer);
                // SAF 插件可能重建了文件，删除任务时应使用最新的真实 URI。
                if is_saf {
                    *controller.final_path.lock().await = saf_file_uri.clone();
                }
                // 这个响应是完整文件，重新打开后仍需正常读取流和写入。
                total
            }
            ResponseAction::InvalidRange => {
                progress_sink.error(&ctx.task_id, "续传响应范围不匹配，请重试");
                break 'download;
            }
            ResponseAction::AlreadyComplete => {
                // 416 沿用原有完成语义，先把缓冲区写入磁盘。
                if let Some(ref mut writer) = file {
                    if let Err(error) = writer.flush() {
                        log::error!("刷新文件缓冲区失败: {}", error);
                        progress_sink
                            .error(&ctx.task_id, &format!("刷新文件缓冲区失败: {}", error));
                        break 'download;
                    }
                }
                completed_ok = true;
                break 'download;
            }
            ResponseAction::LinkExpired => {
                progress_sink.link_expired(&ctx.task_id, downloaded);
                break 'download;
            }
            ResponseAction::HttpError(status) => {
                log::error!("任务 {} 服务器错误: {}", ctx.task_id, status);
                progress_sink.error(&ctx.task_id, &format!("服务器错误: HTTP {}", status));
                break 'download;
            }
            ResponseAction::Stream { total } => total,
        };

        // 流读取、解密、写入和速度采样在共享核心中执行；writer 可以来自普通路径或 SAF。
        let Some(writer) = file.as_mut() else {
            progress_sink.error(&ctx.task_id, "下载文件句柄已失效");
            break 'download;
        };
        let outcome = write_response_stream(
            response,
            writer,
            &mut downloaded,
            total,
            &decrypt_ctx,
            &controller,
            progress_sink,
            &ctx.task_id,
            &mut stream_retries,
        )
        .await;
        match outcome {
            StreamOutcome::Completed => {
                log::info!("下载完成: {}", download_dir);
                completed_ok = true;
                break 'download;
            }
            StreamOutcome::Paused => {
                if controller.wait_for_resume_or_cancel().await {
                    break 'download;
                }
                // 恢复后重新获取链接；关闭旧句柄后，下次按当前偏移打开文件。
                url.clear();
                key.clear();
                file = None;
                continue 'download;
            }
            StreamOutcome::Retry => {
                // 断流后旧链接可能已过期，下一轮重新请求链接再续传。
                url.clear();
                key.clear();
                file = None;
                continue 'download;
            }
            StreamOutcome::Canceled | StreamOutcome::Failed => break 'download,
        }
    }

    // 显式关闭文件句柄，释放资源
    drop(file);

    // 下载成功后，根据设置决定是否写入 metadata（歌词/封面）
    if completed_ok {
        // 文件传输结束，先通知 Rust 任务状态进入 processing，再执行收尾处理。
        progress_sink.file_complete(&ctx.task_id);

        // 歌词、封面和标签由运行时适配器处理；失败只产生警告，不影响音频完成状态。
        let postprocess_request = PostprocessRequest {
            context: &ctx,
            audio_path: &download_dir,
            is_saf,
            saf_file_uri: saf_file_uri.as_deref(),
            saf_folder_uri: saf_folder_uri.as_deref(),
            config: &config,
        };
        if let Some(path) = postprocessor
            .process(postprocess_request, progress_sink)
            .await
        {
            *controller.lrc_final_path.lock().await = Some(path);
        }

        // 全部处理完成，发送下载完成事件
        let final_display_path = if is_saf {
            saf_file_uri.clone().unwrap_or_else(|| download_dir.clone())
        } else {
            download_dir.clone()
        };
        progress_sink.completed(&ctx.task_id, &final_display_path, saf_folder_uri.clone());
    }

    // 如果任务被取消且用户要求删除文件，通过平台文件接口清理普通路径或 SAF URI。
    if controller.cancel_token.is_cancelled()
        && controller.delete_file_on_cancel.load(Ordering::SeqCst)
    {
        let path_to_delete = if is_saf {
            saf_file_uri.as_deref()
        } else {
            Some(download_dir.as_str())
        };
        if let Some(path) = path_to_delete {
            if let Err(error) = file_deleter.remove(path).await {
                log::error!("删除文件失败: {}", error);
            } else {
                log::info!("文件已成功删除: {}", path);
            }
        } else {
            // SAF 文件可能尚未创建；此时没有有效 URI，也就没有可删除的文件。
            log::warn!("任务 {} 取消时未记录 SAF 文件 URI，无法删除", ctx.task_id);
        }
    }

    completed_ok
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex as StdMutex};

    use futures_util::future::BoxFuture;
    use tokio::sync::{Mutex, Notify};
    use tokio_util::sync::CancellationToken;

    use super::download_task;
    use crate::download_config::DownloadConfig;
    use crate::download_link::DownloadLinkProvider;
    use crate::engine::TaskController;
    use crate::local_download_file::LocalDownloadFileOpener;
    use crate::local_file_deleter::LocalFileDeleter;
    use crate::platform::Platform;
    use crate::ports::{DownloadProgressSink, NoopDownloadPostprocessor};
    use crate::task_context::{SongInfo, TaskContext};

    struct RejectedLink;

    impl DownloadLinkProvider for RejectedLink {
        fn fetch<'a>(
            &'a self,
            _platform: Platform,
            _song_mid: &'a str,
            _filename: &'a str,
        ) -> BoxFuture<'a, Result<(String, String), String>> {
            Box::pin(async { Err("平台拒绝".to_string()) })
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        errors: StdMutex<Vec<String>>,
    }

    impl DownloadProgressSink for RecordingSink {
        fn progress(&self, _task_id: &str, _downloaded: u64, _total: u64, _speed: u64) {}
        fn file_complete(&self, _task_id: &str) {}
        fn completed(&self, _task_id: &str, _final_path: &str, _saf_folder_uri: Option<String>) {}
        fn error(&self, _task_id: &str, message: &str) {
            self.errors.lock().unwrap().push(message.to_string());
        }
        fn link_expired(&self, _task_id: &str, _current_offset: u64) {}
        fn metadata_error(&self, _task_id: &str, _message: &str) {}
    }

    #[tokio::test]
    async fn worker_reports_link_rejection_without_tauri() {
        let file_path = std::env::temp_dir().join("hotdownloader-worker-test.mp3");
        let context = TaskContext {
            task_id: "test-task".into(),
            platform: Platform::QqMusic,
            song_mid: "song-mid".into(),
            song_id: 1,
            url: String::new(),
            save_path: file_path.to_string_lossy().to_string(),
            quality: "320kmp3".into(),
            key: String::new(),
            file_size: 100,
            downloaded_offset: 0,
            song_info: SongInfo {
                title: "歌曲".into(),
                artist: "歌手".into(),
                album: String::new(),
                quality: "320kmp3".into(),
                cover_url: String::new(),
            },
            quality_filename: "song.mp3".into(),
            final_path: Arc::new(Mutex::new(None)),
        };
        let controller = TaskController {
            cancel_token: CancellationToken::new(),
            pause_flag: Arc::new(AtomicBool::new(false)),
            resume_notify: Arc::new(Notify::new()),
            url_ready: Arc::new(Notify::new()),
            delete_file_on_cancel: Arc::new(AtomicBool::new(false)),
            final_path: Arc::new(Mutex::new(None)),
            lrc_final_path: Arc::new(Mutex::new(None)),
            started: Arc::new(AtomicBool::new(true)),
            done: Arc::new(Notify::new()),
        };
        let config = DownloadConfig {
            download_dir: std::env::temp_dir().to_string_lossy().to_string(),
            naming_template: "{song} - {artist}".into(),
            saf_folder_uri: None,
            write_metadata: false,
            download_lrc: false,
        };
        let sink = RecordingSink::default();

        // 文件和链接端口均使用与 Tauri 无关的实现，验证核心能独立执行失败状态流转。
        let completed = download_task(
            context,
            controller,
            config,
            &RejectedLink,
            &sink,
            &LocalDownloadFileOpener,
            &LocalFileDeleter,
            &NoopDownloadPostprocessor,
        )
        .await;
        assert!(!completed);
        assert_eq!(
            sink.errors.lock().unwrap().as_slice(),
            ["获取下载链接失败: 平台拒绝"]
        );
    }
}

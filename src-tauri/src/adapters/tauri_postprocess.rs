//! Tauri 收尾适配器：共享核心决定获取和写入内容，这里只桥接 Android SAF。

use std::path::Path;

use futures_util::future::BoxFuture;
use hotdownloader_core::ports::{DownloadPostprocessor, DownloadProgressSink, PostprocessRequest};
use hotdownloader_core::postprocess::{prepare_assets, write_audio_metadata, write_local_lrc};
use tauri::AppHandle;

use crate::download::task_lrc::write_saf_lrc_file;
use crate::download::task_metadata::write_saf_metadata;

pub struct TauriDownloadPostprocessor {
    app: AppHandle,
}

impl TauriDownloadPostprocessor {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DownloadPostprocessor for TauriDownloadPostprocessor {
    fn process<'a>(
        &'a self,
        request: PostprocessRequest<'a>,
        progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move {
            let context = request.context;
            let config = request.config;
            let assets = prepare_assets(context, config.write_metadata, config.download_lrc).await;

            if config.write_metadata {
                if request.is_saf {
                    // SAF 需要应用持有的目录权限；标签内容及编码规则仍由核心写入。
                    write_saf_metadata(
                        &self.app,
                        progress_sink,
                        context,
                        request.audio_path,
                        request.saf_file_uri,
                        &assets,
                    );
                } else if let Err(error) =
                    write_audio_metadata(Path::new(request.audio_path), context, &assets)
                {
                    log::warn!("任务 {} 写入 metadata 失败: {error}", context.task_id);
                    progress_sink.metadata_error(&context.task_id, &error);
                }
            }

            if config.download_lrc {
                if let Some(lyric) = assets.lyric.as_ref() {
                    if request.is_saf {
                        if let Some(content) = lyric
                            .lrc
                            .as_deref()
                            .filter(|content| !content.trim().is_empty())
                        {
                            return write_saf_lrc_file(
                                &self.app,
                                content,
                                request.audio_path,
                                request.saf_folder_uri.map(str::to_string),
                            )
                            .await;
                        }
                    } else {
                        return write_local_lrc(request.audio_path, lyric);
                    }
                }
                log::info!("无普通歌词，跳过 LRC 文件创建");
            }
            None
        })
    }
}

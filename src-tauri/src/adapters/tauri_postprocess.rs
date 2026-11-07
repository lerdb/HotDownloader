//! Tauri 收尾适配器：共享核心决定获取和写入内容，这里只桥接 Android SAF。

use std::path::Path;

use futures_util::future::BoxFuture;
use hotdownloader_core::adapters::local::postprocess::{
    write_audio_metadata, write_local_cover, write_local_lrc,
};
use hotdownloader_core::download::ports::{
    DownloadPostprocessor, DownloadProgressSink, PostprocessRequest,
};
use hotdownloader_core::download::postprocess::{cover_file_name, lrc_file_name, prepare_assets};
use tauri::AppHandle;

use crate::download::task_metadata::write_saf_metadata;
use crate::download::task_sidecar::write_saf_sidecar_file;

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
    ) -> BoxFuture<'a, Vec<String>> {
        Box::pin(async move {
            let context = request.context;
            let config = request.config;
            let assets = prepare_assets(context, config).await;
            let mut paths = Vec::new();

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

            if config.download_cover {
                if let Some(bytes) = assets.cover_bytes.as_deref() {
                    if request.is_saf {
                        if let Some(name) = cover_file_name(request.audio_path, bytes) {
                            paths.extend(write_saf_sidecar_file(
                                &self.app,
                                &name,
                                bytes,
                                request.saf_folder_uri,
                            ));
                        }
                    } else {
                        paths.extend(write_local_cover(request.audio_path, bytes));
                    }
                } else {
                    log::info!("无可用封面，跳过封面文件创建");
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
                            paths.extend(write_saf_sidecar_file(
                                &self.app,
                                &lrc_file_name(request.audio_path),
                                content.as_bytes(),
                                request.saf_folder_uri,
                            ));
                        }
                    } else {
                        paths.extend(write_local_lrc(request.audio_path, lyric));
                    }
                } else {
                    log::info!("无普通歌词，跳过 LRC 文件创建");
                }
            }
            paths
        })
    }
}

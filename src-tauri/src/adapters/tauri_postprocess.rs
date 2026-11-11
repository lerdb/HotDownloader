//! Tauri 收尾适配器：共享核心决定获取和写入内容，这里只桥接 Android SAF。

use futures_util::future::BoxFuture;
use hotdownloader_core::adapters::local::postprocess::LocalDownloadPostprocessor;
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
    ) -> BoxFuture<'a, Result<Vec<String>, String>> {
        Box::pin(async move {
            if !request.is_saf {
                return LocalDownloadPostprocessor
                    .process(request, progress_sink)
                    .await;
            }
            let context = request.context;
            let config = request.config;
            let assets = prepare_assets(context, config).await?;
            let mut paths = Vec::new();

            if config.write_metadata {
                if request.is_saf {
                    // SAF 需要应用持有的目录权限；标签内容及编码规则仍由核心写入。
                    write_saf_metadata(
                        &self.app,
                        context,
                        request.audio_path,
                        request.saf_file_uri,
                        &assets,
                    )?;
                }
            }

            if config.download_cover {
                if let Some(bytes) = assets.cover_bytes.as_deref() {
                    if request.is_saf {
                        if let Some(name) = cover_file_name(request.audio_path, bytes) {
                            paths.push(
                                write_saf_sidecar_file(
                                    &self.app,
                                    &name,
                                    bytes,
                                    request.saf_folder_uri,
                                )
                                .ok_or("保存 SAF 封面失败")?,
                            );
                        }
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
                            paths.push(
                                write_saf_sidecar_file(
                                    &self.app,
                                    &lrc_file_name(request.audio_path),
                                    content.as_bytes(),
                                    request.saf_folder_uri,
                                )
                                .ok_or("保存 SAF 歌词失败")?,
                            );
                        }
                    }
                } else {
                    log::info!("无普通歌词，跳过 LRC 文件创建");
                }
            }
            Ok(paths)
        })
    }
}

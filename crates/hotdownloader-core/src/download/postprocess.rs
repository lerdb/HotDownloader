//! 下载后的歌词、封面获取与附属文件命名，供普通文件和 SAF 收尾流程共用。

use std::path::Path;

use lofty::picture::Picture;

use crate::download::config::DownloadConfig;
use crate::download::context::TaskContext;
use crate::download::cover::fetch_cover_bytes;
use crate::platforms::lyric::LyricData;
use crate::platforms::Platform;

/// 一次获取的歌词与封面会同时供音频标签及独立文件使用。
pub struct PostprocessAssets {
    pub lyric: Option<LyricData>,
    pub cover_bytes: Option<Vec<u8>>,
}

/// 独立歌词沿用音频文件名的 stem；普通路径与 Android SAF 使用同一命名规则。
/// 文件名无法转换为 UTF-8 时沿用 `unknown.lrc`，保持已有下载结果格式。
pub fn lrc_file_name(audio_path: &str) -> String {
    sidecar_file_name(audio_path, "lrc")
}

/// 封面沿用音频文件名的 stem，扩展名根据实际图片格式确定。
pub fn cover_file_name(audio_path: &str, cover_bytes: &[u8]) -> Option<String> {
    let picture = Picture::from_reader(&mut &cover_bytes[..]).ok()?;
    Some(sidecar_file_name(audio_path, picture.mime_type()?.ext()?))
}

fn sidecar_file_name(audio_path: &str, extension: &str) -> String {
    let stem = Path::new(audio_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown");
    format!("{stem}.{extension}")
}

/// 根据任务配置准备收尾数据。平台接口失败只影响对应的可选内容。
pub async fn prepare_assets(context: &TaskContext, config: &DownloadConfig) -> PostprocessAssets {
    let lyric = if config.write_metadata || config.download_lrc {
        let result = match context.platform {
            Platform::Netease => {
                crate::platforms::netease::lyrics::get_lyric_by_id(context.song_id).await
            }
            Platform::Kugou => crate::platforms::kugou::lyrics::get_lyric(&context.song_mid).await,
            Platform::QqMusic => {
                crate::platforms::qqmusic::lyrics::get_lyric_by_id(context.song_id).await
            }
            Platform::Kuwo => {
                crate::platforms::kuwo::lyrics::get_lyric_by_id(context.song_id).await
            }
        };
        match result {
            Ok(lyric) => Some(lyric),
            Err(error) => {
                log::warn!("获取歌词失败: {error}");
                None
            }
        }
    } else {
        None
    };

    // 酷我搜索结果可能缺封面。写标签或单独保存封面时才补取。
    let needs_cover = config.write_metadata || config.download_cover;
    let mut cover_url = context.song_info.cover_url.clone();
    if needs_cover && cover_url.is_empty() && matches!(context.platform, Platform::Kuwo) {
        match crate::platforms::kuwo::cover::fetch_cover(context.song_id).await {
            Ok(url) => cover_url = url,
            Err(error) => log::warn!("任务 {} 获取酷我封面失败: {error}", context.task_id),
        }
    }

    let cover_bytes = if needs_cover && !cover_url.is_empty() {
        fetch_cover_bytes(context.platform, &cover_url, config.cover_quality).await
    } else {
        None
    };

    PostprocessAssets { lyric, cover_bytes }
}

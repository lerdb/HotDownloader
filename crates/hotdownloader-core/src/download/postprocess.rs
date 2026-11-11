//! 下载后的歌词、封面获取与附属文件命名，供普通文件和 SAF 收尾流程共用。

use std::path::Path;

use lofty::picture::Picture;

use crate::download::config::DownloadConfig;
use crate::download::context::TaskContext;
use crate::download::cover::fetch_cover_bytes;
use crate::platforms::lyric::LyricData;
use crate::platforms::Platform;

pub const POSTPROCESS_FAILED: &str = "下载收尾失败";

/// 每项资源独立重试，已成功获取的资源不会因另一项失败而重新请求。
pub(crate) async fn retry_asset<T, F, Fut>(name: &str, mut fetch: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut last = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
        }
        match fetch().await {
            Ok(value) => return Ok(value),
            Err(error) => last = error,
        }
    }
    Err(format!("获取{name}失败（已尝试 3 次）: {last}"))
}

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

/// 所有已启用的资源准备完成后才进入写入阶段。平台明确返回无歌词时允许为空。
pub async fn prepare_assets(
    context: &TaskContext,
    config: &DownloadConfig,
) -> Result<PostprocessAssets, String> {
    let lyric = if config.write_metadata || config.download_lrc {
        let lyric = retry_asset("歌词", || async {
            match context.platform {
                Platform::Netease => {
                    crate::platforms::netease::lyrics::get_lyric_by_id(context.song_id).await
                }
                Platform::Kugou => {
                    crate::platforms::kugou::lyrics::get_lyric(&context.song_mid).await
                }
                Platform::QqMusic => {
                    crate::platforms::qqmusic::lyrics::get_lyric_by_id(context.song_id).await
                }
                Platform::Kuwo => {
                    crate::platforms::kuwo::lyrics::get_lyric_by_id(context.song_id).await
                }
            }
        })
        .await?;
        Some(lyric)
    } else {
        None
    };

    // 酷我搜索结果可能缺封面。写标签或单独保存封面时才补取。
    let needs_cover = config.write_metadata || config.download_cover;
    let mut cover_url = context.song_info.cover_url.clone();
    if needs_cover && cover_url.is_empty() && matches!(context.platform, Platform::Kuwo) {
        cover_url = retry_asset("封面地址", || {
            crate::platforms::kuwo::cover::fetch_cover(context.song_id)
        })
        .await?;
    }

    let cover_bytes = if needs_cover && !cover_url.is_empty() {
        Some(
            retry_asset("封面", || async {
                fetch_cover_bytes(context.platform, &cover_url, config.cover_quality)
                    .await
                    .ok_or_else(|| "所有封面地址均不可用或返回了无效图片".to_string())
            })
            .await?,
        )
    } else if config.download_cover {
        return Err("歌曲未提供封面地址，无法保存封面".into());
    } else {
        None
    };

    Ok(PostprocessAssets { lyric, cover_bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn transient_asset_failure_retries_but_exhaustion_returns_error() {
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let result = retry_asset("歌词", || async {
            if attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 2 {
                Err("HTTP 429".into())
            } else {
                Ok("lyrics")
            }
        })
        .await;
        assert_eq!(result.unwrap(), "lyrics");
        assert_eq!(attempts.load(std::sync::atomic::Ordering::Relaxed), 3);
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let result = retry_asset::<(), _, _>("封面", || async {
            attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Err("invalid image".into())
        })
        .await;
        assert!(result.unwrap_err().contains("invalid image"));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::Relaxed), 3);
    }
}

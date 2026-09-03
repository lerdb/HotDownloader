//! 搜索命令路由层

use crate::platforms::Platform;
use tauri::{command, AppHandle};

#[command]
pub async fn search_songs(
    app: AppHandle,
    platform: String,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => {
            crate::platforms::qqmusic::search::search_songs(&app, keyword, page, limit).await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::search::search_songs(&app, keyword, page, limit).await
        }
    }
}

/// 按歌曲 ID 获取封面 URL（酷我专用）。
///
/// 搜索接口不再并发拉封面，改为按需调用此命令。返回的 URL 可直接用于 `<img :src>`。
/// QQ 音乐场景下歌曲已自带 coverUrl，不应调用此命令。
#[command]
pub async fn fetch_cover(platform: String, song_id: u64) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => Err("QQ 音乐封面已包含在搜索结果中，无需单独获取".into()),
        Platform::Kuwo => crate::platforms::kuwo::cover::fetch_cover(song_id).await,
    }
}
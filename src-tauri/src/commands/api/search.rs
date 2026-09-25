//! 搜索命令路由层

use crate::platforms::Platform;
use crate::utils::filename::get_artist_separator;
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
    // 只在 IPC 边界读取 Tauri 设置；核心搜索函数只接收普通分隔符文本。
    let separator = get_artist_separator(&app);
    match p {
        Platform::QqMusic => {
            crate::platforms::qqmusic::search::search_songs(&separator, keyword, page, limit).await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::search::search_songs(&separator, keyword, page, limit).await
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

#[command]
pub async fn search_albums(
    app: AppHandle,
    platform: String,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    if page == 0 || limit == 0 || limit > 100 {
        return Err("无效的分页参数".into());
    }
    let separator = get_artist_separator(&app);
    match Platform::from_str(&platform)? {
        Platform::QqMusic => {
            crate::platforms::qqmusic::search::search_albums(&separator, keyword, page, limit).await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::search::search_albums(&separator, keyword, page, limit).await
        }
    }
}

#[command]
pub async fn fetch_album_songs(
    app: AppHandle,
    platform: String,
    id: String,
) -> Result<String, String> {
    let separator = get_artist_separator(&app);
    match Platform::from_str(&platform)? {
        Platform::QqMusic => {
            crate::platforms::qqmusic::album::fetch_album_songs(&separator, id).await
        }
        Platform::Kuwo => crate::platforms::kuwo::album::fetch_album_songs(&separator, id).await,
    }
}

#[command]
pub async fn search_artists(
    platform: String,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    validate_artist_page(page, limit)?;
    match Platform::from_str(&platform)? {
        Platform::QqMusic => {
            crate::platforms::qqmusic::search::search_artists(keyword, page, limit).await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::search::search_artists(keyword, page, limit).await
        }
    }
}

#[command]
pub async fn fetch_artist_songs(
    app: AppHandle,
    platform: String,
    id: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    validate_artist_page(page, limit)?;
    let separator = get_artist_separator(&app);
    match Platform::from_str(&platform)? {
        Platform::QqMusic => {
            crate::platforms::qqmusic::artist::fetch_artist_songs(&separator, id, page, limit).await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::artist::fetch_artist_songs(&separator, id, page, limit).await
        }
    }
}

#[command]
pub async fn fetch_artist_albums(
    app: AppHandle,
    platform: String,
    id: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    validate_artist_page(page, limit)?;
    let separator = get_artist_separator(&app);
    match Platform::from_str(&platform)? {
        Platform::QqMusic => {
            crate::platforms::qqmusic::artist::fetch_artist_albums(&separator, id, page, limit)
                .await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::artist::fetch_artist_albums(&separator, id, page, limit).await
        }
    }
}

fn validate_artist_page(page: u32, limit: u32) -> Result<(), String> {
    if page == 0 || limit == 0 || limit > 100 {
        return Err("无效的分页参数".into());
    }
    Ok(())
}

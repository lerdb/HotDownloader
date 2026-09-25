//! 歌单导入命令路由层

use crate::platforms::Platform;
use crate::utils::filename::get_artist_separator;
use tauri::{command, AppHandle};

#[command]
pub async fn fetch_playlist_songs(
    app: AppHandle,
    platform: String,
    input: String,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    // 歌单解析需要用户设置的歌手分隔符，具体请求无需接触 AppHandle。
    let separator = get_artist_separator(&app);
    match p {
        Platform::QqMusic => {
            crate::platforms::qqmusic::playlist::fetch_playlist_songs(&separator, input).await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::playlist::fetch_playlist_songs(&separator, input).await
        }
    }
}

/// 搜索歌单命令路由层。
///
/// 接收平台标识、搜索关键词、页码（从 1 开始）和每页数量，调用对应平台的歌单搜索实现。
/// 返回 JSON 字符串，格式为 `{"playlists": [...], "has_more": true/false}`。
#[command]
pub async fn search_playlists(
    platform: String,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => {
            crate::platforms::qqmusic::playlist::search_playlists(keyword, page, limit).await
        }
        Platform::Kuwo => {
            crate::platforms::kuwo::playlist::search_playlists(keyword, page, limit).await
        }
    }
}

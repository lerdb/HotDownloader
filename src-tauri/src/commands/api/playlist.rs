//! 歌单导入命令路由层

use crate::platforms::Platform;
use tauri::command;

#[command]
pub async fn fetch_playlist_songs(platform: String, input: String) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => crate::platforms::qqmusic::playlist::fetch_playlist_songs(input).await,
    }
}

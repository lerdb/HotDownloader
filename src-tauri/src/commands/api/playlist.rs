//! 歌单导入命令路由层

use crate::platforms::Platform;
use tauri::{command, AppHandle};

#[command]
pub async fn fetch_playlist_songs(
    app: AppHandle,
    platform: String,
    input: String,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => {
            crate::platforms::qqmusic::playlist::fetch_playlist_songs(&app, input).await
        }
        Platform::Kuwo => Err("该平台暂不支持歌单导入".into()),
    }
}

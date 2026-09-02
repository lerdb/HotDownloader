//! 搜索命令路由层

use crate::platforms::Platform;
use tauri::command;

#[command]
pub async fn search_songs(
    platform: String,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => {
            crate::platforms::qqmusic::search::search_songs(keyword, page, limit).await
        }
    }
}

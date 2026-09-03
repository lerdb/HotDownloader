//! 热搜与搜索建议命令路由层

use crate::platforms::Platform;
use tauri::command;

#[command]
pub async fn fetch_hot_keywords(platform: String) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => crate::platforms::qqmusic::suggest::fetch_hot_keywords().await,
        Platform::Kuwo => crate::platforms::kuwo::suggest::fetch_hot_keywords().await,
    }
}

#[command]
pub async fn fetch_suggestions(platform: String, keyword: String) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => crate::platforms::qqmusic::suggest::fetch_suggestions(keyword).await,
        Platform::Kuwo => crate::platforms::kuwo::suggest::fetch_suggestions(keyword).await,
    }
}

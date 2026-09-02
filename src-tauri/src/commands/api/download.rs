//! 下载链接获取命令路由层

use crate::platforms::Platform;
use tauri::{command, AppHandle};

/// 内部函数：根据平台获取下载链接与解密密钥，返回原始元组。
/// 供下载模块直接调用，避免平台判断散落。
pub(crate) async fn fetch_download_link_inner(
    app_handle: &AppHandle,
    platform: Platform,
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    match platform {
        Platform::QqMusic => {
            crate::platforms::qqmusic::download::get_download_link(app_handle, song_mid, filename)
                .await
        }
    }
}

/// Tauri 命令：获取下载链接和密钥，返回 JSON 字符串。
#[command]
pub async fn fetch_download_link(
    app: AppHandle,
    platform: String,
    song_mid: String,
    filename: String,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    let (url, key) = fetch_download_link_inner(&app, p, &song_mid, &filename).await?;
    Ok(serde_json::json!({ "url": url, "key": key }).to_string())
}

use std::time::Duration;

use tauri::AppHandle;

use crate::commands::api::download; // 获取下载链接
use crate::platforms::Platform;

/// 重试获取下载链接（网络错误时最多尝试 3 次）
/// 传入 AppHandle，使下载链接获取函数能够读取登录态
pub(crate) async fn fetch_download_link_with_retry(
    app_handle: &AppHandle,
    song_mid: &str,
    filename: &str,
    task_id: &str,
    platform: Platform,
) -> Result<(String, String), String> {
    let mut last_err = String::new();
    for attempt in 0..3 {
        // 调用命令层内部函数，平台判断在内部完成
        let result =
            download::fetch_download_link_inner(app_handle, platform, song_mid, filename).await;

        match result {
            Ok(link) => return Ok(link),
            Err(e) => {
                last_err = e.clone();
                if !is_retryable_link_error(&e) {
                    log::warn!("任务 {} 获取下载链接失败: {}", task_id, e);
                    return Err(e);
                }
                log::warn!(
                    "任务 {} 获取下载链接失败 (尝试 {}/3): {}",
                    task_id,
                    attempt + 1,
                    last_err
                );
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                    // 1s, 2s, 4s
                }
            }
        }
    }
    Err(last_err)
}

/// 判断错误是否属于可重试的网络类错误
pub(crate) fn is_retryable_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || (err.is_request() && !err.is_body())
}

/// 判断获取下载链接时的错误是否属于可重试的网络类错误
/// 可重试错误包括：网络错误、读取响应失败、解析响应失败（均为临时性问题）
/// 平台拒绝类错误（如 104003）不可重试，直接返回
fn is_retryable_link_error(err: &str) -> bool {
    err.starts_with("网络错误")
        || err.starts_with("读取响应失败")
        || err.starts_with("解析响应失败")
}

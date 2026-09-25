//! 下载链接获取命令路由层

use crate::platforms::Platform;
use tauri::AppHandle;

/// 内部函数：根据平台获取下载链接与解密密钥，返回原始元组。
/// 供下载模块直接调用，避免平台判断散落。
///
/// # 参数
/// - `app_handle`: Tauri 应用句柄（用于读取登录态）。
/// - `platform`: 平台枚举。
/// - `song_mid`: 歌曲 ID 字符串（QQ 用 mid，酷我用数字 ID 字符串）。
/// - `filename`: 品质对应的文件名。
///   - QQ 用作真实文件名（如 `M800xxxx.mp3`）；
///   - 酷我用作 `{bitrate}.{format}` 形式（如 `320.mp3`、`20900.mflac`），
///     模块内部从扩展名解析 format、从 stem 解析 bitrate。
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
        Platform::Kuwo => {
            crate::platforms::kuwo::download::get_download_link(app_handle, song_mid, filename)
                .await
        }
    }
}

use std::io::{Seek, Write};
use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_android_fs::{AndroidFsExt, FileAccessMode, FsUri};

/// 通过 Android SAF 保存歌词或封面，已有同名文件会被清空后重写。
/// 普通文件路径由共享核心处理；失败返回 None，由收尾适配器报告错误。
pub(crate) fn write_saf_sidecar_file(
    app_handle: &AppHandle,
    file_name: &str,
    content: &[u8],
    saf_folder_uri: Option<&str>,
) -> Option<String> {
    let parent_uri = match saf_folder_uri {
        Some(s) => match FsUri::from_json_str(s) {
            Ok(uri) => uri,
            Err(e) => {
                log::warn!("解析 SAF 文件夹 URI 失败: {}", e);
                return None;
            }
        },
        None => {
            log::warn!("SAF 文件夹 URI 缺失，无法创建附属文件 {file_name}");
            return None;
        }
    };

    let api = app_handle.android_fs();

    let file_path = Path::new(file_name);
    let file_uri_opt = api.resolve_file_uri(&parent_uri, file_path).ok();

    match file_uri_opt {
        Some(existing_uri) => {
            // 文件已存在：打开并清空后写入
            match api.open_file(&existing_uri, FileAccessMode::ReadWrite) {
                Ok(mut f) => {
                    if let Err(e) = f.set_len(0) {
                        log::warn!("清空附属文件 {file_name} 失败: {e}");
                        return None;
                    }
                    if let Err(e) = f.seek(std::io::SeekFrom::Start(0)) {
                        log::warn!("附属文件 {file_name} seek 失败: {e}");
                        return None;
                    }
                    if let Err(e) = f.write_all(content) {
                        log::warn!("写入附属文件 {file_name} 失败: {e}");
                        return None;
                    }
                    log::info!("SAF 附属文件已保存: {}", existing_uri.uri);
                    Some(existing_uri.uri.clone())
                }
                Err(e) => {
                    log::warn!("打开已有附属文件 {file_name} 失败: {e}");
                    None
                }
            }
        }
        None => {
            // 文件不存在：创建新文件
            match api.create_new_file(&parent_uri, file_path, None) {
                Ok(new_uri) => match api.open_file_writable(&new_uri) {
                    Ok(mut f) => {
                        if let Err(e) = f.write_all(content) {
                            log::warn!("写入附属文件 {file_name} 失败: {e}");
                            return None;
                        }
                        log::info!("SAF 附属文件已保存: {}", new_uri.uri);
                        Some(new_uri.uri.clone())
                    }
                    Err(e) => {
                        log::warn!("打开新附属文件 {file_name} 失败: {e}");
                        None
                    }
                },
                Err(e) => {
                    log::warn!("创建附属文件 {file_name} 失败: {e}");
                    None
                }
            }
        }
    }
}

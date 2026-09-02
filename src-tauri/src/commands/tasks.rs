use crate::download::engine::DownloadEngine;
use crate::platforms::Platform;
use crate::storage::store_wrapper;
use serde_json::json;
use tauri::{command, AppHandle, Manager};
use tauri_plugin_android_fs::{AndroidFsExt, FsUri};

#[command]
pub fn load_tasks(app: AppHandle) -> Result<String, String> {
    store_wrapper::load_string(&app, "tasks").map_err(|e| e.to_string())
}

#[command]
pub fn save_tasks(app: AppHandle, tasks_json: String) -> Result<(), String> {
    store_wrapper::save_string(&app, "tasks", &tasks_json).map_err(|e| e.to_string())
}

#[command]
pub async fn add_download_task(
    app: AppHandle,
    task_id: String,
    platform: String,
    song_id: u64,
    song_mid: String,
    url: String,
    save_path: String,
    quality: String,
    filename: String,
    key: String,
    file_size: u64,
    song_title: String,
    artist: String,
    album: String,
    cover_url: String,
) -> Result<(), String> {
    let p = Platform::from_str(&platform)?;
    let engine = app.state::<DownloadEngine>();
    engine
        .add_task(
            task_id, p, song_id, song_mid, url, save_path, quality, filename, key, file_size,
            song_title, artist, album, cover_url,
        )
        .await;
    Ok(())
}

#[command]
pub async fn enqueue_task(app: AppHandle, task_id: String, offset: u64) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>().clone();
    engine.enqueue_task(&task_id, offset).await;
    Ok(())
}

#[command]
pub async fn pause_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>().clone();
    engine.pause(&task_id).await;
    Ok(())
}

#[command]
pub async fn resume_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>().clone();
    engine.resume(&task_id).await;
    Ok(())
}

#[command]
pub async fn cancel_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>().clone();
    engine.cancel(&task_id, delete_file).await;
    Ok(())
}

#[command]
pub async fn remove_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>().clone();
    engine.remove(&task_id, delete_file).await
}

#[command]
pub fn set_max_concurrent(app: AppHandle, max: u32) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>();
    engine.set_concurrency(max);
    Ok(())
}
#[command]
pub async fn check_download_path(
    app: AppHandle,
    _song_id: u64,
    _song_mid: String,
    song_title: String,
    artist: String,
    album: String,
    cover_url: String,
    quality_filename: String,
    quality: String,
) -> Result<String, String> {
    // 构建 SongInfo 对象，quality 必须传入，否则命名模板中 {quality} 会出错
    let song_info = crate::download::task::SongInfo {
        title: song_title,
        artist,
        album,
        quality,
        cover_url,
    };
    // 先获取下载设置，再调用路径解析函数，避免函数内部再次读取设置
    let (dir_setting, template_setting, saf_uri_setting, _, _) =
        crate::download::task_path::get_download_settings(&app).await;
    let (is_saf, download_dir, saf_folder_uri) = crate::download::task_path::resolve_download_path(
        &dir_setting,
        &template_setting,
        saf_uri_setting.as_deref(),
        &song_info,
        &quality_filename,
    );

    // 检查文件是否存在
    let exists = if is_saf {
        if let Some(parent_uri_str) = saf_folder_uri.as_ref() {
            match FsUri::from_json_str(parent_uri_str) {
                Ok(parent_uri) => {
                    let file_path = std::path::Path::new(&download_dir);
                    let api = app.android_fs();
                    api.resolve_file_uri(&parent_uri, file_path).is_ok()
                }
                Err(_) => false,
            }
        } else {
            false
        }
    } else {
        std::path::Path::new(&download_dir).exists()
    };
    // 生成建议的重命名路径（若存在），找到第一个不存在的路径后直接返回结果
    if exists {
        let path = std::path::Path::new(&download_dir);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let mut counter = 1;
        loop {
            let new_name = if ext.is_empty() {
                format!("{} ({})", stem, counter)
            } else {
                format!("{} ({}).{}", stem, counter, ext)
            };
            let new_path = if is_saf {
                new_name.clone()
            } else {
                path.parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(&new_name)
                    .to_string_lossy()
                    .to_string()
            };
            // 检查新路径是否存在
            let new_exists = if is_saf {
                if let Some(parent_uri_str) = saf_folder_uri.as_ref() {
                    match FsUri::from_json_str(parent_uri_str) {
                        Ok(parent_uri) => {
                            let file_path = std::path::Path::new(&new_name);
                            let api = app.android_fs();
                            api.resolve_file_uri(&parent_uri, file_path).is_ok()
                        }
                        Err(_) => false,
                    }
                } else {
                    false
                }
            } else {
                std::path::Path::new(&new_path).exists()
            };
            if !new_exists {
                // 找到不存在的路径，直接返回包含建议路径的 JSON
                return Ok(json!({
                    "original_path": download_dir,
                    "exists": exists,
                    "suggested_path": new_path,
                    "is_saf": is_saf,
                })
                .to_string());
            }
            counter += 1;
        }
        // 循环理论上总能找到不存在的路径，不会执行到这里
    }

    // 文件不存在时，suggested_path 返回原始路径
    Ok(json!({
        "original_path": download_dir,
        "exists": exists,
        "suggested_path": download_dir,
        "is_saf": is_saf,
    })
    .to_string())
}

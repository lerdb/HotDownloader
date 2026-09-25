use crate::download::contract::{
    BatchResult, CreateTaskRequest, CreateTaskResult, DuplicateAction, TaskRecord, TaskStatus,
};
use crate::download::engine::DownloadEngine;
use crate::download::task_rules::TaskRules;
use crate::download::task_state::TaskState;
use crate::platforms::Platform;
use crate::storage::store_wrapper;
use tauri::{AppHandle, Manager};
use tauri_plugin_android_fs::{AndroidFsExt, FsUri};

// 创建、控制、重试都由这里编排。Tauri command 只负责把 IPC 参数转发进来，
// 页面关闭后下载器仍能依靠 TaskState 和自身的调度循环继续工作。
pub async fn load_tasks(app: AppHandle) -> Result<Vec<TaskRecord>, String> {
    Ok(app.state::<TaskState>().list())
}

fn task_rules(app: &AppHandle) -> TaskRules {
    // 每次执行创建/重试命令时读取最新设置，前端只需先刷新设置到存储。
    let raw = store_wrapper::load_string(app, "settings").unwrap_or_default();
    let settings = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    TaskRules::from_settings(&settings)
}

async fn register_engine_task(app: &AppHandle, task: &TaskRecord) -> Result<(), String> {
    // 持久化记录是下载器上下文的来源。重试旧任务时可用同一记录重建上下文，
    // 无需依赖前端页面保留 URL、路径或下载参数。
    let platform = Platform::from_str(&task.platform)?;
    let engine = app.state::<DownloadEngine>();
    engine
        .add_task(
            task.id.clone(),
            platform,
            task.song_id,
            task.song_mid.clone(),
            String::new(),
            task.save_path.clone().unwrap_or_default(),
            task.quality.clone(),
            task.filename.clone(),
            String::new(),
            task.file_size,
            task.song_title.clone(),
            task.artist.clone(),
            task.album.clone(),
            task.cover_url.clone(),
            task.downloaded,
        )
        .await;
    Ok(())
}

struct PathCheck {
    original_path: String,
    exists: bool,
    suggested_path: String,
    is_saf: bool,
}

async fn resume_offset(app: &AppHandle, task: &TaskRecord) -> u64 {
    // 内存中的 downloaded 可能来自上次进度事件，断点续传必须以实际文件长度为准。
    // SAF 路径不能直接用 std::fs 读取；此处回退到 0，让下载器从头处理。
    let path = match task.save_path.as_deref() {
        Some(path) if std::path::Path::new(path).is_absolute() => Some(path.to_string()),
        _ => task_path_check(app, task)
            .await
            .ok()
            .filter(|result| !result.is_saf)
            .map(|result| result.original_path),
    };
    // 文件大小超过预期总量时不能继续追加，否则可能损坏目标文件。
    path.and_then(|path| std::fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .filter(|size| task.file_size == 0 || *size <= task.file_size)
        .unwrap_or(0)
}

fn candidate_exists(app: &AppHandle, path: &str, is_saf: bool) -> bool {
    if !is_saf {
        return std::path::Path::new(path).exists();
    }
    // Android SAF 的“路径”是文件名，需要在用户授权的目录 URI 下解析。
    let raw = store_wrapper::load_string(app, "settings").unwrap_or_default();
    let settings: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
    let Some(uri) = settings["safFolderUri"].as_str() else {
        return false;
    };
    let Ok(parent) = FsUri::from_json_str(uri) else {
        return false;
    };
    app.android_fs()
        .resolve_file_uri(&parent, std::path::Path::new(path))
        .is_ok()
}

fn unique_path(
    app: &AppHandle,
    original: &str,
    suggested: &str,
    is_saf: bool,
    state: &TaskState,
) -> Result<String, String> {
    // 同时检查磁盘与尚未创建文件的排队任务，避免两个任务取得同一目标。
    if !state.path_reserved(suggested) && !candidate_exists(app, suggested, is_saf) {
        return Ok(suggested.to_string());
    }
    let path = std::path::Path::new(original);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("无效文件名")?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for n in 1..10_000 {
        let name = if ext.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        let candidate = if is_saf {
            name
        } else {
            path.parent()
                .ok_or("无效下载目录")?
                .join(name)
                .to_string_lossy()
                .to_string()
        };
        if !state.path_reserved(&candidate) && !candidate_exists(app, &candidate, is_saf) {
            return Ok(candidate);
        }
    }
    Err("无法生成不重复的文件名".into())
}

pub async fn create_download_task(
    app: AppHandle,
    request: CreateTaskRequest,
) -> Result<CreateTaskResult, String> {
    Platform::from_str(&request.song.platform)?;
    let state = app.state::<TaskState>();
    // 锁覆盖路径检查、占用和入队，防止并发创建时绕过重名判断。
    let _creation_guard = state.creation_lock.lock().await;
    let rules = task_rules(&app);
    let selected = rules
        .initial_quality(&request.desired_quality, &request.song.qualities)
        .cloned();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let id = format!("{now:x}-{:x}", rand::random::<u64>());
    // 即使音质不可用，也建立可见的错误任务，供用户查看失败原因。
    let mut task = TaskRecord {
        id,
        platform: request.song.platform,
        song_id: request.song.id,
        song_mid: request.song.mid,
        song_title: request.song.title,
        artist: request.song.artist,
        album: request.song.album,
        cover_url: request.song.cover_url,
        media_mid: request.song.media_mid,
        filename: selected
            .as_ref()
            .map_or_else(String::new, |q| q.filename.clone()),
        quality: selected
            .as_ref()
            .map_or(request.desired_quality, |q| q.quality.clone()),
        status: if selected.is_some() {
            TaskStatus::Waiting
        } else {
            TaskStatus::Error
        },
        error_msg: if selected.is_some() {
            None
        } else {
            Some("所选音质不可用".into())
        },
        file_path: None,
        file_size: selected.as_ref().map_or(0, |q| q.size),
        downloaded: 0,
        retry_count: 0,
        added_at: now,
        available_qualities: Some(request.song.qualities),
        speed: None,
        save_path: None,
    };

    if selected.is_some() {
        let path = task_path_check(&app, &task).await?;
        let reserved = state.path_reserved(&path.original_path);
        if path.exists || reserved {
            // 请求中的显式选择优先于默认策略；ask 返回结果让 UI 弹窗，
            // 用户决定后再调用同一个命令，不能由 UI 自己计算重命名路径。
            let action = request
                .duplicate_action
                .or(match rules.duplicate_strategy.as_str() {
                    "overwrite" => Some(DuplicateAction::Overwrite),
                    "rename" => Some(DuplicateAction::Rename),
                    "cancel" => Some(DuplicateAction::Cancel),
                    _ => None,
                });
            match action {
                None => {
                    return Ok(CreateTaskResult::NeedsConfirmation {
                        song_title: task.song_title,
                    })
                }
                Some(DuplicateAction::Cancel) => return Ok(CreateTaskResult::Cancelled),
                Some(DuplicateAction::Rename) => {
                    task.save_path = Some(unique_path(
                        &app,
                        &path.original_path,
                        &path.suggested_path,
                        path.is_saf,
                        &state,
                    )?)
                }
                Some(DuplicateAction::Overwrite) if reserved => {
                    // 正在使用的目标文件不能覆盖，否则会破坏另一任务的下载。
                    return Err("同名任务正在下载，请选择保留两份".into());
                }
                Some(DuplicateAction::Overwrite) => task.save_path = Some(path.original_path),
            }
        } else {
            task.save_path = Some(path.original_path);
        }
    }

    // 先持久化，再交给下载器；窗口关闭或注册失败时仍能看到任务及错误。
    state.insert(task.clone())?;
    if task.status == TaskStatus::Waiting {
        if let Err(e) = register_engine_task(&app, &task).await {
            app.state::<TaskState>().failed(&task.id, &e, None);
            return Err(e);
        }
    }
    Ok(CreateTaskResult::Created { task })
}

pub async fn pause_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let state = app.state::<TaskState>();
    let _creation_guard = state.creation_lock.lock().await;
    let task = state.get(&task_id).ok_or("任务不存在")?;
    if task.status != TaskStatus::Downloading && task.status != TaskStatus::Waiting {
        return Err("只有等待或下载中的任务可以暂停".into());
    }
    // 先让下载器停止工作，再更新权威状态，避免 UI 先显示暂停但仍在写文件。
    let engine = app.state::<DownloadEngine>().clone();
    engine.pause(&task_id).await;
    state.update(&task_id, true, |task| {
        task.status = TaskStatus::Paused;
        task.speed = None;
    })?;
    Ok(())
}

pub async fn resume_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let state = app.state::<TaskState>();
    let _creation_guard = state.creation_lock.lock().await;
    if state.get(&task_id).ok_or("任务不存在")?.status != TaskStatus::Paused {
        return Err("只有暂停的任务可以恢复".into());
    }
    let engine = app.state::<DownloadEngine>().clone();
    engine.resume(&task_id).await;
    state.update(&task_id, true, |task| task.status = TaskStatus::Downloading)?;
    Ok(())
}

pub async fn cancel_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>().clone();
    let state = app.state::<TaskState>();
    let _creation_guard = state.creation_lock.lock().await;
    let task = state.get(&task_id).ok_or("任务不存在")?;
    // 已完成任务可能不在下载器活动表中，删除文件时仍须把最终路径交给引擎。
    if let Some(path) = task.file_path.as_deref().or_else(|| {
        task.save_path
            .as_deref()
            .filter(|path| std::path::Path::new(path).is_absolute())
    }) {
        engine.remember_final_path(&task_id, path).await;
    }
    if let Err(e) = engine.remove(&task_id, delete_file).await {
        if task.status != TaskStatus::Completed {
            state.failed(&task_id, &e, None);
        }
        return Err(e);
    }
    state.remove(&task_id)
}

pub async fn remove_task(app: AppHandle, task_id: String, delete_file: bool) -> Result<(), String> {
    cancel_task(app, task_id, delete_file).await
}

/// 批量移除任务。用于“清除所选”等场景，
/// 避免前端逐个 invoke（每个任务一次 IPC + 一次整表写盘）导致的卡顿。
pub async fn remove_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
    delete_file: bool,
) -> Result<BatchResult, String> {
    let mut errors: Vec<String> = Vec::new();
    // 让前端获得后端真实成功/失败数量，用于清除完成后的准确通知。
    let mut succeeded: usize = 0;
    let mut failed: usize = 0;
    for task_id in task_ids {
        if let Err(e) = cancel_task(app.clone(), task_id.clone(), delete_file).await {
            log::error!("批量移除任务失败 {}: {}", task_id, e);
            // 批量移除失败时保留 task_id，便于前端/日志定位具体失败任务。
            errors.push(format!("{}: {}", task_id, e));
            failed += 1;
        } else {
            succeeded += 1;
        }
    }
    Ok(BatchResult {
        succeeded,
        failed,
        errors,
    })
}

pub async fn retry_task(app: AppHandle, task_id: String) -> Result<bool, String> {
    let state = app.state::<TaskState>();
    let _creation_guard = state.creation_lock.lock().await;
    let mut task = state.get(&task_id).ok_or("任务不存在")?;
    if task.status != TaskStatus::Error {
        return Ok(false);
    }
    let engine = app.state::<DownloadEngine>().clone();
    // 上一轮 worker 可能刚发出失败事件但尚未退出；先等待，避免新旧 worker 并发写入。
    engine.wait_for_task_exit(&task_id).await;
    let rules = task_rules(&app);
    let next_count = task.retry_count + 1;
    let mut downgraded = false;
    if next_count > 3 {
        // 次数耗尽后的行为完全由 Rust 决定；旧任务没有品质列表时不能凭空推断文件名。
        if !rules.auto_downgrade {
            state.update(&task_id, true, |t| {
                t.retry_count = next_count;
                t.error_msg = Some("重试次数已用尽".into());
            })?;
            return Ok(false);
        }
        let available = match task.available_qualities.as_deref() {
            Some(available) => available,
            None => {
                state.update(&task_id, true, |t| {
                    t.retry_count = next_count;
                    t.error_msg = Some("旧任务缺少可用品质信息，请重新添加下载".into());
                })?;
                return Ok(false);
            }
        };
        let next = match rules.next_quality(&task.quality, available) {
            Some(next) => next.clone(),
            None => {
                state.update(&task_id, true, |t| {
                    t.retry_count = next_count;
                    t.error_msg = Some("已无后续可用品质".into());
                })?;
                return Ok(false);
            }
        };
        task.quality = next.quality;
        task.filename = next.filename;
        task.file_size = next.size;
        task.downloaded = 0;
        task.retry_count = 0;
        task.save_path = None;
        downgraded = true;
        // 降级后的文件名和大小都已变更，需要重新计算路径并解决冲突。
        let path = task_path_check(&app, &task).await?;
        let reserved = state.path_reserved(&path.original_path);
        if path.exists || reserved {
            match rules.duplicate_strategy.as_str() {
                "cancel" => {
                    state.update(&task_id, true, |t| {
                        t.error_msg = Some("降级目标文件已存在".into());
                    })?;
                    return Ok(false);
                }
                "overwrite" if !reserved => task.save_path = Some(path.original_path),
                // 重试可能由批量操作触发，不能等待浏览器弹窗；ask 时自动保留两份。
                _ => {
                    task.save_path = Some(unique_path(
                        &app,
                        &path.original_path,
                        &path.suggested_path,
                        path.is_saf,
                        &state,
                    )?)
                }
            }
        } else {
            task.save_path = Some(path.original_path);
        }
    } else {
        // 同品质重试保留目标路径，但从磁盘重新测量已下载字节数。
        task.retry_count = next_count;
        task.downloaded = resume_offset(&app, &task).await;
    }

    // 先提交等待状态及本次重试参数，前端随后只接收状态快照。
    state.update(&task_id, true, |record| {
        record.quality = task.quality.clone();
        record.filename = task.filename.clone();
        record.file_size = task.file_size;
        record.downloaded = task.downloaded;
        record.retry_count = task.retry_count;
        record.save_path = task.save_path.clone();
        record.error_msg = None;
        record.status = TaskStatus::Waiting;
    })?;

    let result = if downgraded {
        // 品质改变后，旧下载器上下文中的 URL、目标路径都不再适用。
        engine.forget_final_path(&task_id).await;
        register_engine_task(&app, &task).await
    } else {
        match engine.enqueue_task(&task_id, task.downloaded).await {
            Ok(()) => Ok(()),
            Err(e) if e.contains(crate::download::engine::ERR_TASK_CONTEXT_MISSING) => {
                // 应用重启后只有持久化记录，没有进程内下载器上下文，直接重建。
                register_engine_task(&app, &task).await
            }
            Err(e) => Err(e),
        }
    };
    if let Err(e) = result {
        state.failed(&task_id, &format!("启动下载失败: {e}"), None);
        return Err(e);
    }
    Ok(true)
}

pub fn set_max_concurrent(app: AppHandle, max: u32) -> Result<(), String> {
    let engine = app.state::<DownloadEngine>();
    engine.set_concurrency(max);
    Ok(())
}

async fn task_path_check(app: &AppHandle, task: &TaskRecord) -> Result<PathCheck, String> {
    // 构建 SongInfo 对象，quality 必须传入，否则命名模板中 {quality} 会出错
    let song_info = crate::download::task::SongInfo {
        title: task.song_title.clone(),
        artist: task.artist.clone(),
        album: task.album.clone(),
        quality: task.quality.clone(),
        cover_url: task.cover_url.clone(),
    };
    // 先获取下载设置，再调用路径解析函数，避免函数内部再次读取设置
    let (dir_setting, template_setting, saf_uri_setting, _, _) =
        crate::download::task_path::get_download_settings(&app).await;
    let (is_saf, download_dir, saf_folder_uri) = crate::download::task_path::resolve_download_path(
        &dir_setting,
        &template_setting,
        saf_uri_setting.as_deref(),
        &song_info,
        &task.filename,
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
                // 路径结果在服务层直接使用类型，避免为了内部调用序列化再反序列化。
                return Ok(PathCheck {
                    original_path: download_dir,
                    exists,
                    suggested_path: new_path,
                    is_saf,
                });
            }
            counter += 1;
        }
        // 循环理论上总能找到不存在的路径，不会执行到这里
    }

    // 文件不存在时，suggested_path 返回原始路径
    Ok(PathCheck {
        original_path: download_dir.clone(),
        exists,
        suggested_path: download_dir,
        is_saf,
    })
}

use std::fs;
use std::io::{BufWriter, Seek};
use std::path::Path;

use hotdownloader_core::local_download_file::open_local_download_file;
use hotdownloader_core::ports::DownloadProgressSink;
use tauri::AppHandle;
use tauri_plugin_android_fs::{AndroidFsExt, FileAccessMode, FsUri};

/// 文件写入缓冲区容量（64 KB）。
///
/// 用于 `BufWriter`，在写入磁盘前暂存数据，减少频繁的 I/O 操作。
const FILE_BUFFER_CAPACITY: usize = 64 * 1024;

/// 打开或创建下载目标文件，并返回带缓冲的写入器。
///
/// 根据是否使用 Android SAF（Storage Access Framework）来决定调用相应的内部函数。
/// 该函数会处理文件不存在、文件大小异常等情况，并可能在必要时重置 `downloaded` 计数。
///
/// # 参数
/// - `app_handle`: Tauri 应用句柄，仅用于访问 Android FS 插件。
/// - `progress_sink`: 下载状态输出接口，负责记录错误并通知订阅者。
/// - `task_id`: 下载任务的唯一标识，用于错误事件中定位具体任务。
/// - `download_dir`: 下载目标路径。在普通模式下是完整的文件路径；在 SAF 模式下仅作为文件名使用。
/// - `is_saf`: 是否启用 SAF 模式。
/// - `saf_folder_uri`: SAF 模式下父目录的 URI（JSON 字符串形式），仅在 `is_saf` 为 `true` 时有效。
/// - `downloaded`: 可变引用，表示当前已下载的字节数。在文件异常或新文件情况下可能被重置为 0。
/// - `saf_file_uri`: 可变引用，用于记录最终实际使用的 SAF 文件 URI（仅在 SAF 模式下会被更新）。
///
/// # 返回
/// - `Some(BufWriter<fs::File>)`：成功打开或创建文件，调用方可继续写入。
/// - `None`：发生错误（已通过状态接口上报），调用方应中止下载循环。
pub(crate) async fn open_download_file(
    app_handle: &AppHandle,
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
    download_dir: &str,
    is_saf: bool,
    saf_folder_uri: Option<&str>,
    downloaded: &mut u64,
    saf_file_uri: &mut Option<String>,
) -> Option<BufWriter<fs::File>> {
    if is_saf {
        open_saf_file(
            app_handle,
            progress_sink,
            task_id,
            download_dir,
            saf_folder_uri,
            downloaded,
            saf_file_uri,
        )
        .await
    } else {
        open_normal_file(progress_sink, task_id, download_dir, downloaded)
    }
}

/// SAF 模式下打开或创建下载目标文件。
///
/// 使用 Android Storage Access Framework 进行文件操作。
/// 首先解析父目录 URI，然后尝试查找已存在的文件；若存在则根据 `downloaded` 判断是续传还是重置；
/// 若不存在则创建新文件。最终返回一个 `BufWriter` 用于写入。
///
/// # 参数
/// - `app_handle`: Tauri 应用句柄，用于访问 Android FS 插件。
/// - `task_id`: 下载任务唯一标识，用于错误事件。
/// - `download_dir`: 文件名（不含路径），SAF 模式下父目录由 `saf_folder_uri` 指定。
/// - `saf_folder_uri`: 父目录的 URI（JSON 字符串形式），必须为 `Some` 且可解析。
/// - `downloaded`: 可变引用，已下载字节数。根据文件状态可能被重置为 0。
/// - `saf_file_uri`: 可变引用，将被更新为最终文件的实际 URI（无论文件是已存在还是新创建）。
///
/// # 返回
/// - `Some(BufWriter<fs::File>)`：成功打开文件，准备好写入。
/// - `None`：任何一步失败（如 URI 解析错误、文件打开/创建失败、seek 失败等），
///   已通过状态接口发送错误事件。
async fn open_saf_file(
    app_handle: &AppHandle,
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
    download_dir: &str,
    saf_folder_uri: Option<&str>,
    downloaded: &mut u64,
    saf_file_uri: &mut Option<String>,
) -> Option<BufWriter<fs::File>> {
    let api = app_handle.android_fs();

    // 解析父目录 FsUri（包含 document_top_tree_uri 等信息）
    let parent_uri = match FsUri::from_json_str(saf_folder_uri?) {
        Ok(uri) => uri,
        Err(e) => {
            log::error!("解析 SAF 文件夹 URI 失败: {}", e);
            progress_sink.error(task_id, "SAF 配置错误");
            return None;
        }
    };

    // 在 SAF 模式下，`download_dir` 仅表示文件名
    let file_path = Path::new(download_dir);

    // 尝试解析已存在的文件
    let existing_file_uri = api.resolve_file_uri(&parent_uri, file_path).ok();

    match existing_file_uri {
        Some(file_uri) => {
            // 文件已存在，记录最终文件 URI
            *saf_file_uri = Some(file_uri.uri.clone());

            if *downloaded > 0 {
                // 续传模式：以读写模式打开文件，并校验大小后 seek 到偏移量
                match api.open_file(&file_uri, FileAccessMode::ReadWrite) {
                    Ok(mut f) => {
                        // 校验文件大小：如果文件长度小于期望的偏移，说明文件异常，需要重置下载
                        let should_reset = match f.metadata() {
                            Ok(meta) => meta.len() < *downloaded,
                            Err(_) => true, // 无法获取元数据，保守重置
                        };

                        if should_reset {
                            log::warn!(
                                "任务 {} SAF 文件大小异常，重置下载（期望偏移 {}，实际大小 {}）",
                                task_id,
                                *downloaded,
                                f.metadata().map(|m| m.len()).unwrap_or(0)
                            );
                            // 清空文件并从头下载
                            if let Err(e) = f.set_len(0) {
                                log::error!("SAF 文件截断失败: {}", e);
                                progress_sink.error(task_id, "文件异常，请重试");
                                return None;
                            }
                            if let Err(e) = f.seek(std::io::SeekFrom::Start(0)) {
                                log::error!("SAF 文件 seek 失败: {}", e);
                                progress_sink.error(task_id, "文件定位失败");
                                return None;
                            }
                            *downloaded = 0;
                        } else {
                            // 文件大小正常，seek 到续传位置
                            if let Err(e) = f.seek(std::io::SeekFrom::Start(*downloaded)) {
                                log::error!("SAF 文件 seek 失败: {}", e);
                                progress_sink.error(task_id, "文件定位失败");
                                return None;
                            }
                        }
                        Some(f)
                    }
                    Err(e) => {
                        log::error!("SAF 打开文件失败: {}", e);
                        progress_sink.error(task_id, "无法打开文件");
                        None
                    }
                }
            } else {
                // 从头开始：以可写模式打开并截断文件
                match api.open_file_writable(&file_uri) {
                    Ok(f) => Some(f),
                    Err(e) => {
                        log::error!("SAF 打开文件失败: {}", e);
                        progress_sink.error(task_id, "无法打开文件");
                        None
                    }
                }
            }
        }
        None => {
            // 文件不存在，创建新文件
            match api.create_new_file(&parent_uri, file_path, None) {
                Ok(file_uri) => {
                    *saf_file_uri = Some(file_uri.uri.clone());
                    // 新文件，重置偏移量
                    if *downloaded > 0 {
                        *downloaded = 0;
                    }
                    match api.open_file_writable(&file_uri) {
                        Ok(f) => Some(f),
                        Err(e) => {
                            log::error!("SAF 打开新文件失败: {}", e);
                            progress_sink.error(task_id, "无法打开文件");
                            None
                        }
                    }
                }
                Err(e) => {
                    log::error!("SAF 创建文件失败: {}", e);
                    progress_sink.error(task_id, "无法创建文件");
                    None
                }
            }
        }
    }
    .map(|f| BufWriter::with_capacity(FILE_BUFFER_CAPACITY, f))
}

/// 普通文件系统模式下打开或创建下载目标文件。
///
/// 普通文件的创建、续传校验和重置交给共享核心；此处只转发错误到现有事件接口。
///
/// # 参数
/// - `progress_sink`: 下载状态输出接口，用于记录错误并通知订阅者。
/// - `task_id`: 下载任务唯一标识，用于错误事件。
/// - `download_dir`: 完整的本地文件路径。
/// - `downloaded`: 可变引用，已下载字节数。文件异常时可能被重置为 0。
///
/// # 返回
/// - `Some(BufWriter<fs::File>)`：成功打开文件。
/// - `None`：文件创建/打开失败，或续传时文件异常且重置失败。
fn open_normal_file(
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
    download_dir: &str,
    downloaded: &mut u64,
) -> Option<BufWriter<fs::File>> {
    match open_local_download_file(download_dir, downloaded) {
        Ok(file) => Some(file),
        Err(error) => {
            log::error!("{error}");
            progress_sink.error(task_id, error.user_message);
            None
        }
    }
}

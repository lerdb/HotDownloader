use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::tag::{ItemKey, Tag};
use once_cell::sync::Lazy;
use reqwest::header::{CONTENT_LENGTH, RANGE};
use reqwest::StatusCode;
use tauri::AppHandle;
use tauri_plugin_android_fs::{AndroidFsExt, FileAccessMode, FsUri};
use tokio::sync::Mutex;

use super::engine::TaskController;
use super::progress;
use crate::commands::api::{self}; // 获取下载链接
use crate::utils::{crypto, filename};

/// 文件写入缓冲区容量（64 KB）
const FILE_BUFFER_CAPACITY: usize = 64 * 1024;

/// 下载专用 HTTP 客户端：不设总超时，避免大文件下载中断；设置读取超时 5 分钟
static DOWNLOAD_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("HotDownloader/1.0")
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(300)) // 5 分钟读取超时
        .build()
        .expect("Failed to create download HTTP client")
});

/// 歌曲信息，用于生成文件名
#[derive(Clone)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub quality: String,
    pub cover_url: String, // 封面 URL，用于写入音频标签
}

/// 单个任务的上下文信息
#[derive(Clone)]
pub struct TaskContext {
    pub task_id: String,
    pub song_mid: String, // 歌曲字符串标识（QQ音乐 mid）
    pub song_id: u64,     // 歌曲数字 ID
    pub url: String,
    pub save_path: String, // 最终文件路径
    #[allow(dead_code)] // 抑制未使用警告，保留备用
    pub quality: String,
    #[allow(dead_code)] // 抑制未使用警告，保留备用
    pub key: String,
    pub file_size: u64,
    pub downloaded_offset: u64,
    #[allow(dead_code)] // 抑制未使用警告，保留备用
    pub app_handle: AppHandle,
    pub song_info: SongInfo,
    pub quality_filename: String,
    pub final_path: Arc<Mutex<Option<String>>>, // 与控制器共享的文件路径
}

/// 重试获取下载链接（仅网络错误最多尝试 3 次，平台拒绝立即返回）
async fn fetch_download_link_with_retry(
    song_mid: &str,
    filename: &str,
    task_id: &str,
) -> Result<(String, String), String> {
    let mut last_err = String::new();
    for attempt in 0..3 {
        match api::get_download_link(song_mid, filename).await {
            Ok(link) => return Ok(link),
            Err(e) => {
                last_err = e;
                log::warn!(
                    "任务 {} 获取下载链接失败 (尝试 {}/3): {}",
                    task_id,
                    attempt + 1,
                    last_err
                );
                if crate::utils::link_error::is_unavailable_link_error(&last_err) {
                    return Err(last_err);
                }
                if !crate::utils::link_error::is_retryable_link_error(&last_err) {
                    return Err(last_err);
                }
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                }
            }
        }
    }
    Err(last_err)
}

/// 判断错误是否属于可重试的网络类错误
fn is_retryable_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || (err.is_request() && !err.is_body())
}

/// 将歌词与封面写入音频文件 metadata
/// 普通模式直接操作文件路径；SAF 模式通过临时文件回写实现跨平台支持
/// 所有错误仅记录日志，不阻止下载完成事件的发送
async fn write_metadata(
    app_handle: &AppHandle,
    song_id: u64,
    file_path: &str,
    is_saf: bool,
    saf_file_uri: Option<String>,
    cover_url: &str,
) {
    // 1. 获取歌词：优先逐字歌词，其次普通歌词
    let lyric_text = match crate::commands::lyrics::get_lyric_by_id(song_id).await {
        Ok(resp) => {
            // 优先级：逐字歌词（elrc） → 普通歌词（lrc）
            if let Some(elrc) = resp.elrc.filter(|s| !s.trim().is_empty()) {
                Some(elrc)
            } else if let Some(lrc) = resp.lrc.filter(|s| !s.trim().is_empty()) {
                Some(lrc)
            } else {
                None
            }
        }
        Err(e) => {
            log::warn!("获取歌词失败: {}", e);
            None
        }
    };

    // 2. 下载封面图片字节
    let cover_bytes = if !cover_url.is_empty() {
        match api::CLIENT.get(cover_url).send().await {
            Ok(resp) if resp.status().is_success() => resp.bytes().await.ok().map(|b| b.to_vec()),
            _ => None,
        }
    } else {
        None
    };

    if lyric_text.is_none() && cover_bytes.is_none() {
        log::info!("无可用歌词或封面，跳过 metadata 写入");
        return;
    }

    // 3. 准备本地临时路径：SAF 需先复制到临时文件
    let temp_path = if is_saf {
        let uri = match &saf_file_uri {
            Some(u) => u.clone(),
            None => {
                log::warn!("SAF 文件 URI 缺失，无法写入 metadata");
                return;
            }
        };
        let fs_uri = FsUri::from_uri(uri);
        let api = app_handle.android_fs();

        // 读取 SAF 文件并写入临时文件
        let mut src = match api.open_file(&fs_uri, FileAccessMode::Read) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("打开 SAF 文件读取失败: {}", e);
                return;
            }
        };
        let mut buf = Vec::new();
        if let Err(e) = src.read_to_end(&mut buf) {
            log::warn!("读取 SAF 文件失败: {}", e);
            return;
        }
        // 从原始文件名提取扩展名，保证临时文件能被 lofty 正确识别格式
        // 例如原始文件为 "xxx.flac"，则临时文件使用 ".flac" 后缀
        let ext = Path::new(file_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("tmp");
        let temp = std::env::temp_dir().join(format!(
            "{}.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            ext
        ));
        if let Err(e) = std::fs::write(&temp, &buf) {
            log::warn!("写入临时文件失败: {}", e);
            return;
        }
        temp
    } else {
        PathBuf::from(file_path)
    };

    // 4. 修改 metadata
    let mut tagged_file = match lofty::read_from_path(&temp_path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("读取音频文件失败: {}", e);
            if is_saf {
                let _ = std::fs::remove_file(&temp_path);
            }
            return;
        }
    };

    // 确保存在主标签
    let tag_type = tagged_file.primary_tag_type();
    if tagged_file.primary_tag().is_none() {
        // 没有主标签时创建对应类型的空标签
        let new_tag = Tag::new(tag_type);
        tagged_file.insert_tag(new_tag);
    }

    let tag = match tagged_file.primary_tag_mut() {
        Some(t) => t,
        None => {
            log::warn!("无法获取音频标签，跳过写入");
            if is_saf {
                let _ = std::fs::remove_file(&temp_path);
            }
            return;
        }
    };

    // 写入歌词
    if let Some(lyric) = lyric_text {
        tag.remove_key(&ItemKey::Lyrics);
        tag.insert_text(ItemKey::Lyrics, lyric.clone());
    }

    // 写入封面
    if let Some(bytes) = cover_bytes {
        let picture = Picture::new_unchecked(
            PictureType::CoverFront,
            Some(lofty::picture::MimeType::Jpeg),
            None,
            bytes,
        );
        // 移除旧的封面图片，避免重复
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(picture);
    }

    // 保存 metadata
    if let Err(e) = tagged_file.save_to_path(&temp_path, WriteOptions::default()) {
        log::warn!("保存 metadata 失败: {}", e);
    } else {
        log::info!("metadata 已写入: {}", temp_path.display());
    }

    // 5. SAF 模式：将临时文件写回原文件
    if is_saf {
        if let Some(uri) = saf_file_uri {
            let fs_uri = FsUri::from_uri(uri);
            let api = app_handle.android_fs();
            match api.open_file(&fs_uri, FileAccessMode::ReadWrite) {
                Ok(mut dst) => {
                    let data = match std::fs::read(&temp_path) {
                        Ok(d) => d,
                        Err(e) => {
                            log::warn!("读取临时文件失败: {}", e);
                            let _ = std::fs::remove_file(&temp_path);
                            return;
                        }
                    };
                    // 清空原文件并从头写入，避免旧数据残留
                    if let Err(e) = dst.set_len(0) {
                        log::warn!("清空 SAF 文件失败: {}", e);
                        let _ = std::fs::remove_file(&temp_path);
                        return;
                    }
                    if let Err(e) = dst.seek(std::io::SeekFrom::Start(0)) {
                        log::warn!("SAF 文件 seek 失败: {}", e);
                        let _ = std::fs::remove_file(&temp_path);
                        return;
                    }
                    if let Err(e) = dst.write_all(&data) {
                        log::warn!("写入 SAF 文件失败: {}", e);
                    }
                }
                Err(e) => {
                    log::warn!("打开 SAF 文件写入失败: {}", e);
                }
            }
        }
        let _ = std::fs::remove_file(&temp_path);
    }
}

/// 等待暂停恢复，返回 true 表示任务已被取消，应退出下载循环
async fn wait_for_resume_async(controller: &TaskController) -> bool {
    loop {
        controller.resume_notify.notified().await;
        if !controller.pause_flag.load(Ordering::SeqCst) {
            return false; // 恢复
        }
        if controller.cancel_token.is_cancelled() {
            return true; // 取消
        }
    }
}

/// 实际执行下载的函数
pub async fn download_task(ctx: TaskContext, controller: TaskController, app_handle: AppHandle) {
    // 获取设置并增加 writeMetadata
    let (dir_setting, template_setting, saf_uri_setting, write_metadata_enabled) =
        get_download_settings(&app_handle).await;
    // 1. 构建最终保存路径（只需一次）
    let (is_saf, download_dir, saf_folder_uri) = {
        if !ctx.save_path.is_empty() {
            (false, ctx.save_path.clone(), None)
        } else {
            let dir = dir_setting.clone();
            let template = template_setting.clone();
            let saf_uri = saf_uri_setting.clone();
            if dir == "saf://" && cfg!(target_os = "android") && saf_uri.is_some() {
                let fname = filename::build_filename(&template, &ctx.song_info);
                let raw_ext = Path::new(&ctx.quality_filename)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("flac");
                let ext = map_decrypted_extension(raw_ext);
                let file_name = format!("{}.{}", fname, ext);
                (true, file_name, saf_uri)
            } else {
                let fname = filename::build_filename(&template, &ctx.song_info);
                let raw_ext = Path::new(&ctx.quality_filename)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("flac");
                // 映射解密后的扩展名
                let ext = map_decrypted_extension(raw_ext);
                let full_path = Path::new(&dir).join(format!("{}.{}", fname, ext));
                (false, full_path.to_string_lossy().to_string(), None)
            }
        }
    };

    log::info!("任务 {} 开始下载，文件路径: {}", ctx.task_id, download_dir);

    // 2. 创建目录并验证（仅普通模式需要）
    if !is_saf {
        let parent_dir = Path::new(&download_dir).parent().unwrap_or(Path::new("."));
        if !parent_dir.exists() {
            if let Err(e) = fs::create_dir_all(parent_dir) {
                log::error!("创建下载目录失败: {}", e);
                progress::emit_error(&app_handle, &ctx.task_id, "下载目录无法访问");
                return;
            }
        }
    }

    // 3. 初始化已下载偏移量
    let mut downloaded = ctx.downloaded_offset;

    // 4. 链接与解密密钥（每次循环可能重新获取）
    let mut url = String::new();
    let mut key = String::new();

    // 5. 文件句柄（使用 BufWriter 提升写入性能）
    let mut file: Option<BufWriter<fs::File>> = None;

    let mut saf_file_uri: Option<String> = None;

    // 流错误重试计数器（防止无限重试）
    let mut stream_retries: u32 = 0;
    const MAX_STREAM_RETRIES: u32 = 2;

    let mut completed_ok = false; // 标记下载是否真正完成

    // 下载循环
    'download: loop {
        // 检查取消
        if controller.cancel_token.is_cancelled() {
            break 'download;
        }

        // 初始暂停等待（使用统一的辅助函数）
        while controller.pause_flag.load(Ordering::SeqCst) {
            if wait_for_resume_async(&controller).await {
                break 'download; // 任务被取消
            }
        }

        // 如果没有有效链接，实时获取（首次进入或暂停恢复后）
        if url.is_empty() {
            match fetch_download_link_with_retry(&ctx.song_mid, &ctx.quality_filename, &ctx.task_id)
                .await
            {
                Ok((new_url, new_key)) => {
                    url = new_url;
                    key = new_key;
                    log::info!("任务 {} 获取到新下载链接", ctx.task_id);
                }
                Err(e) => {
                    log::error!("任务 {} 最终获取下载链接失败: {}", ctx.task_id, e);
                    progress::emit_error(&app_handle, &ctx.task_id, &e);
                    break 'download;
                }
            }
        }

        // 根据是否需要解密创建解密上下文
        let need_decrypt = {
            let ext = Path::new(&ctx.quality_filename)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            ext == "mgg" || ext == "mflac"
        };
        let decrypt_ctx = if need_decrypt && !key.is_empty() {
            crypto::init_decryption(&key, true)
        } else {
            crypto::init_decryption("", false)
        };

        // 打开/续传文件
        if file.is_none() {
            let f = if is_saf {
                // SAF 模式
                let api = app_handle.android_fs();

                // 解析父目录 FsUri（包含 document_top_tree_uri）
                let parent_uri = match FsUri::from_json_str(saf_folder_uri.as_ref().unwrap()) {
                    Ok(uri) => uri,
                    Err(e) => {
                        log::error!("解析 SAF 文件夹 URI 失败: {}", e);
                        progress::emit_error(&app_handle, &ctx.task_id, "SAF 配置错误");
                        break 'download;
                    }
                };

                // download_dir 此时是文件名
                let file_path = std::path::Path::new(&download_dir);

                // 尝试解析已存在的文件
                let existing_file_uri = api.resolve_file_uri(&parent_uri, file_path).ok();

                match existing_file_uri {
                    Some(file_uri) => {
                        // 文件已存在，记录最终文件 URI
                        saf_file_uri = Some(file_uri.uri.clone());

                        if downloaded > 0 {
                            // 续传模式：打开文件并校验大小后 seek 到偏移量
                            match api.open_file(&file_uri, FileAccessMode::ReadWrite) {
                                Ok(mut f) => {
                                    use std::io::Seek;

                                    // 校验文件大小：如果文件长度小于期望的偏移，说明文件异常，重置下载
                                    let should_reset = match f.metadata() {
                                        Ok(meta) => meta.len() < downloaded,
                                        Err(_) => true, // 无法获取元数据，保守重置
                                    };

                                    if should_reset {
                                        log::warn!(
                                            "任务 {} SAF 文件大小异常，重置下载（期望偏移 {}，实际大小 {}）",
                                            ctx.task_id,
                                            downloaded,
                                            f.metadata().map(|m| m.len()).unwrap_or(0)
                                        );
                                        // 清空文件并从头下载
                                        if let Err(e) = f.set_len(0) {
                                            log::error!("SAF 文件截断失败: {}", e);
                                            progress::emit_error(
                                                &app_handle,
                                                &ctx.task_id,
                                                "文件异常，请重试",
                                            );
                                            break 'download;
                                        }
                                        if let Err(e) = f.seek(std::io::SeekFrom::Start(0)) {
                                            log::error!("SAF 文件 seek 失败: {}", e);
                                            progress::emit_error(
                                                &app_handle,
                                                &ctx.task_id,
                                                "文件定位失败",
                                            );
                                            break 'download;
                                        }
                                        downloaded = 0;
                                    } else {
                                        // 文件大小正常，seek 到续传位置
                                        if let Err(e) = f.seek(std::io::SeekFrom::Start(downloaded))
                                        {
                                            log::error!("SAF 文件 seek 失败: {}", e);
                                            progress::emit_error(
                                                &app_handle,
                                                &ctx.task_id,
                                                "文件定位失败",
                                            );
                                            break 'download;
                                        }
                                    }
                                    Some(f)
                                }
                                Err(e) => {
                                    log::error!("SAF 打开文件失败: {}", e);
                                    progress::emit_error(&app_handle, &ctx.task_id, "无法打开文件");
                                    break 'download;
                                }
                            }
                        } else {
                            // 从头开始：截断文件
                            match api.open_file_writable(&file_uri) {
                                Ok(f) => Some(f),
                                Err(e) => {
                                    log::error!("SAF 打开文件失败: {}", e);
                                    progress::emit_error(&app_handle, &ctx.task_id, "无法打开文件");
                                    break 'download;
                                }
                            }
                        }
                    }
                    None => {
                        // 文件不存在，创建新文件
                        match api.create_new_file(&parent_uri, file_path, None) {
                            Ok(file_uri) => {
                                saf_file_uri = Some(file_uri.uri.clone());
                                // 新文件，重置偏移量
                                if downloaded > 0 {
                                    downloaded = 0;
                                }
                                match api.open_file_writable(&file_uri) {
                                    Ok(f) => Some(f),
                                    Err(e) => {
                                        log::error!("SAF 打开新文件失败: {}", e);
                                        progress::emit_error(
                                            &app_handle,
                                            &ctx.task_id,
                                            "无法打开文件",
                                        );
                                        break 'download;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("SAF 创建文件失败: {}", e);
                                progress::emit_error(&app_handle, &ctx.task_id, "无法创建文件");
                                break 'download;
                            }
                        }
                    }
                }
            } else {
                // 普通模式：原有逻辑
                if downloaded == 0 {
                    match OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&download_dir)
                    {
                        Ok(f) => Some(f),
                        Err(e) => {
                            log::error!("文件创建失败: {}", e);
                            progress::emit_error(
                                &app_handle,
                                &ctx.task_id,
                                "文件创建失败，请检查磁盘空间",
                            );
                            break 'download;
                        }
                    }
                } else {
                    // 续传任务，先以追加模式打开
                    match OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&download_dir)
                    {
                        Ok(f) => {
                            // 校验文件大小：如果文件长度小于期望的偏移，说明文件异常，重置下载
                            if let Ok(meta) = f.metadata() {
                                if meta.len() < downloaded {
                                    // 文件被截断或损坏，清空文件并从头下载
                                    drop(f); // 先关闭文件，避免占用
                                    match OpenOptions::new()
                                        .write(true)
                                        .create(true)
                                        .truncate(true)
                                        .open(&download_dir)
                                    {
                                        Ok(new_f) => {
                                            downloaded = 0;
                                            Some(new_f)
                                        }
                                        Err(e) => {
                                            log::error!("文件重置失败: {}", e);
                                            progress::emit_error(
                                                &app_handle,
                                                &ctx.task_id,
                                                "文件异常，请重试",
                                            );
                                            break 'download;
                                        }
                                    }
                                } else {
                                    Some(f)
                                }
                            } else {
                                // 无法获取元数据，保守起见改为从头下载
                                drop(f);
                                match OpenOptions::new()
                                    .write(true)
                                    .create(true)
                                    .truncate(true)
                                    .open(&download_dir)
                                {
                                    Ok(new_f) => {
                                        downloaded = 0;
                                        Some(new_f)
                                    }
                                    Err(e) => {
                                        log::error!("文件重置失败: {}", e);
                                        progress::emit_error(
                                            &app_handle,
                                            &ctx.task_id,
                                            "文件异常，请重试",
                                        );
                                        break 'download;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("文件打开失败: {}", e);
                            progress::emit_error(&app_handle, &ctx.task_id, "文件访问失败");
                            break 'download;
                        }
                    }
                }
            };

            if let Some(f) = f {
                // 包装为 BufWriter，减少磁盘 IO 次数
                file = Some(BufWriter::with_capacity(FILE_BUFFER_CAPACITY, f));

                // 更新 final_path：SAF 模式为 URI，普通模式为普通路径
                if is_saf {
                    if let Some(uri) = saf_file_uri.clone() {
                        *controller.final_path.lock().await = Some(uri);
                    }
                } else {
                    *controller.final_path.lock().await = Some(download_dir.clone());
                }
            } else {
                break 'download;
            }
        }

        // 发起下载请求（带网络重试）
        let mut attempt = 0;
        let response = loop {
            let mut request = DOWNLOAD_CLIENT
                .get(&url)
                .header("Referer", "https://y.qq.com");

            if downloaded > 0 {
                request = request.header(RANGE, format!("bytes={}-", downloaded));
            }
            match request.send().await {
                Ok(resp) => break resp,
                Err(e) => {
                    attempt += 1;
                    log::warn!(
                        "任务 {} 下载请求失败 (尝试 {}/3): {}",
                        ctx.task_id,
                        attempt,
                        e
                    );
                    if is_retryable_network_error(&e) && attempt < 3 {
                        tokio::time::sleep(Duration::from_secs(1 << (attempt - 1))).await;
                        continue;
                    } else {
                        log::error!("任务 {} 最终下载请求失败: {}", ctx.task_id, e);
                        progress::emit_error(&app_handle, &ctx.task_id, "网络错误，请稍后重试");
                        break 'download;
                    }
                }
            }
        };

        // 从响应头获取真实文件总大小
        let total = {
            if let Some(content_range) = response.headers().get("content-range") {
                content_range
                    .to_str()
                    .ok()
                    .and_then(|s| s.split('/').last().and_then(|n| n.parse::<u64>().ok()))
                    .unwrap_or(0)
            } else {
                response
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0)
            }
        };
        let total = if total > 0 { total } else { ctx.file_size };

        let status = response.status();
        if status == StatusCode::RANGE_NOT_SATISFIABLE {
            // 刷新缓冲区
            if let Some(ref mut f) = file {
                if let Err(e) = f.flush() {
                    log::error!("刷新文件缓冲区失败: {}", e);
                    progress::emit_error(&app_handle, &ctx.task_id, "写入文件失败");
                    break 'download;
                }
            }
            completed_ok = true;
            break 'download;
        }

        if status.is_client_error() || status.is_server_error() {
            if status == StatusCode::FORBIDDEN
                || status == StatusCode::GONE
                || status == StatusCode::NOT_FOUND
            {
                progress::emit_link_expired(&app_handle, &ctx.task_id, downloaded);
            } else {
                log::error!("任务 {} 服务器错误: {}", ctx.task_id, status);
                progress::emit_error(&app_handle, &ctx.task_id, "服务器错误，请稍后重试");
            }
            break 'download;
        }

        let mut stream = response.bytes_stream();
        let mut last_report = Instant::now();
        let mut last_downloaded = downloaded;
        let mut should_retry_stream = false;

        // 速度平滑：保存最近 5 次采样的速度值（B/s）
        let mut speed_samples: VecDeque<u64> = VecDeque::with_capacity(5);

        // 内部流读取循环
        loop {
            // 检查取消
            if controller.cancel_token.is_cancelled() {
                break 'download;
            }

            // 检查暂停：如果暂停，跳出内部循环，回到外层重新请求
            if controller.pause_flag.load(Ordering::SeqCst) {
                break; // 暂停跳出内部循环
            }

            let chunk_result = stream.next().await;
            let chunk = match chunk_result {
                Some(Ok(bytes)) => bytes,
                Some(Err(e)) => {
                    log::error!("任务 {} 读取流错误: {}", ctx.task_id, e);
                    // 如果还未超过流错误重试次数，标记为重试并跳出内部循环
                    if stream_retries < MAX_STREAM_RETRIES {
                        stream_retries += 1;
                        should_retry_stream = true;
                    } else {
                        progress::emit_error(&app_handle, &ctx.task_id, "网络错误，请稍后重试");
                    }
                    break;
                }
                None => break, // 流正常结束
            };

            // 转换为可变的 Vec<u8>
            let mut chunk_data = chunk.to_vec();
            let chunk_len = chunk_data.len() as u64;

            // 解密
            crypto::decrypt_chunk(&decrypt_ctx, &mut chunk_data, chunk_len, downloaded);

            // 写入文件
            if let Some(ref mut f) = file {
                if let Err(e) = f.write_all(&chunk_data) {
                    log::error!("写入文件错误: {}", e);
                    progress::emit_error(&app_handle, &ctx.task_id, "写入文件失败");
                    break 'download;
                }
            }

            downloaded += chunk_len;

            let now = Instant::now();
            let elapsed = now - last_report;
            if elapsed >= Duration::from_millis(500) {
                // 计算瞬时速度
                let instant_speed = if elapsed.as_secs_f64() > 0.0 {
                    ((downloaded - last_downloaded) as f64 / elapsed.as_secs_f64()) as u64
                } else {
                    0
                };

                // 加入采样队列并计算移动平均
                speed_samples.push_back(instant_speed);
                if speed_samples.len() > 5 {
                    speed_samples.pop_front();
                }
                let avg_speed = if speed_samples.is_empty() {
                    0
                } else {
                    speed_samples.iter().sum::<u64>() / speed_samples.len() as u64
                };

                progress::emit_progress(&app_handle, &ctx.task_id, downloaded, total, avg_speed);
                last_report = now;
                last_downloaded = downloaded;
            }

            if total > 0 && downloaded >= total {
                // 刷新缓冲区
                if let Some(ref mut f) = file {
                    if let Err(e) = f.flush() {
                        log::error!("刷新文件缓冲区失败: {}", e);
                        progress::emit_error(&app_handle, &ctx.task_id, "写入文件失败");
                        break 'download;
                    }
                }
                log::info!("下载完成: {}", download_dir);
                completed_ok = true;
                break 'download;
            }
        }

        // 内部循环结束后的处理
        if controller.pause_flag.load(Ordering::SeqCst) {
            // 因暂停跳出，等待恢复
            if wait_for_resume_async(&controller).await {
                break 'download; // 被取消
            }
            // 恢复后需要重新获取链接，清空 url 与 key，并释放当前文件句柄
            url.clear();
            key.clear();
            file = None; // 释放 BufWriter 并自动 flush
            continue 'download;
        }

        // 如果是因为流错误触发的重试
        if should_retry_stream {
            // 重新获取链接，避免旧链接过期
            url.clear();
            key.clear();
            file = None; // 释放 BufWriter 并自动 flush
            continue 'download;
        }

        // 其他情况，直接退出
        break 'download;
    }

    // 显式关闭文件句柄，释放资源
    drop(file);

    // 下载成功后，根据设置决定是否写入 metadata（歌词/封面）
    if completed_ok {
        if write_metadata_enabled {
            write_metadata(
                &app_handle,
                ctx.song_id,
                &download_dir,
                is_saf,
                saf_file_uri.clone(),
                &ctx.song_info.cover_url,
            )
            .await;
        }

        // 无论 metadata 是否写入成功，均发送下载完成事件
        let final_display_path = if is_saf {
            saf_file_uri.clone().unwrap_or_else(|| download_dir.clone())
        } else {
            download_dir.clone()
        };
        progress::emit_completed(
            &app_handle,
            &ctx.task_id,
            &final_display_path,
            saf_folder_uri.clone(),
        );
    }

    // 如果任务被取消且用户要求删除文件，执行删除
    if controller.cancel_token.is_cancelled()
        && controller.delete_file_on_cancel.load(Ordering::SeqCst)
    {
        if is_saf {
            // SAF 模式：使用插件 API 删除
            if let Some(uri) = saf_file_uri.clone() {
                let fs_uri = FsUri::from_uri(uri);
                let api = app_handle.android_fs();
                if let Err(e) = api.remove_file(&fs_uri) {
                    log::error!("删除 SAF 文件失败: {}", e);
                } else {
                    log::info!("SAF 文件已删除: {}", fs_uri.uri);
                }
            } else {
                log::warn!("任务 {} 取消时未记录 SAF 文件 URI，无法删除", ctx.task_id);
            }
        } else {
            // 普通模式：使用标准库删除
            if let Err(e) = fs::remove_file(&download_dir) {
                log::error!("删除文件失败: {}", e);
            } else {
                log::info!("文件已成功删除: {}", download_dir);
            }
        }
    }
}

// ==================== 辅助函数 ====================

/// 获取下载目录（绝对路径）及文件命名模板
pub(crate) async fn get_download_settings(
    app_handle: &AppHandle,
) -> (String, String, Option<String>, bool) {
    use crate::storage::store_wrapper;

    let default_dir = crate::commands::file_ops::get_default_download_dir_impl(app_handle);
    let default_template = "{song} - {artist}".to_string();

    let settings_json = store_wrapper::load_string(app_handle, "settings").unwrap_or_default();
    let settings: serde_json::Value =
        serde_json::from_str(&settings_json).unwrap_or(serde_json::json!({}));

    let dir = settings
        .get("downloadDir")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_dir.clone());

    // 过滤无效路径（Android 应用私有目录）
    let dir = if dir.contains("/data/user/0/") || dir.contains("/data/data/") {
        log::warn!("检测到应用私有目录路径，已回退为默认下载目录: {}", dir);
        default_dir
    } else if Path::new(&dir).is_absolute() || dir == "saf://" {
        dir
    } else {
        log::warn!(
            "下载目录不是绝对路径，已回退为默认下载目录: {}",
            default_dir
        );
        default_dir
    };

    let saf_folder_uri = settings
        .get("safFolderUri")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let template = settings
        .get("namingTemplate")
        .and_then(|v| v.as_str())
        .unwrap_or(&default_template)
        .to_string();

    // 是否写入歌曲标签（歌词/封面）
    let write_metadata = settings
        .get("writeMetadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    (dir, template, saf_folder_uri, write_metadata)
}

/// 将加密文件扩展名映射为解密后的真实扩展名
pub(crate) fn map_decrypted_extension(ext: &str) -> &str {
    match ext {
        "mgg" => "ogg",
        "mflac" => "flac",
        // 未知则保持原样
        _ => ext,
    }
}

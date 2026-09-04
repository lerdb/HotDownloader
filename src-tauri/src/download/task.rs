use std::collections::VecDeque;
use std::fs::{self};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use once_cell::sync::Lazy;
use reqwest::header::{CONTENT_LENGTH, RANGE};
use reqwest::StatusCode;
use tauri::AppHandle;
use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
use tokio::sync::Mutex;

use super::engine::TaskController;
use super::progress;
use super::task_fetch::{fetch_download_link_with_retry, is_retryable_network_error};
use super::task_file::open_download_file;
use super::task_lrc::write_lrc_file;
use super::task_metadata::write_metadata;
use super::task_path::{get_download_settings, resolve_download_path};
use crate::commands::api::lyrics;
use crate::platforms::Platform;
use crate::utils::crypto;

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
    pub platform: Platform,
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
// 返回任务是否成功完成，供调度器决定是否清理任务上下文
pub async fn download_task(
    ctx: TaskContext,
    controller: TaskController,
    app_handle: AppHandle,
) -> bool {
    // 获取全部设置，避免在 resolve_download_path 中重复调用
    let (
        dir_setting,
        template_setting,
        saf_uri_setting,
        write_metadata_enabled,
        download_lrc_enabled,
    ) = get_download_settings(&app_handle).await;

    // 1. 构建最终保存路径（只需一次）
    let (is_saf, download_dir, saf_folder_uri) = if !ctx.save_path.is_empty() {
        (false, ctx.save_path.clone(), None)
    } else {
        resolve_download_path(
            &dir_setting,
            &template_setting,
            saf_uri_setting.as_deref(),
            &ctx.song_info,
            &ctx.quality_filename,
        )
    };

    log::info!("任务 {} 开始下载，文件路径: {}", ctx.task_id, download_dir);

    // 2. 创建目录并验证（仅普通模式需要）
    if !is_saf {
        let parent_dir = Path::new(&download_dir).parent().unwrap_or(Path::new("."));
        if !parent_dir.exists() {
            if let Err(e) = fs::create_dir_all(parent_dir) {
                log::error!("创建下载目录失败: {}", e);
                progress::emit_error(&app_handle, &ctx.task_id, "下载目录无法访问");
                return false;
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
            match fetch_download_link_with_retry(
                &app_handle,
                &ctx.song_mid,
                &ctx.quality_filename,
                &ctx.task_id,
                ctx.platform, // 添加平台参数
            )
            .await
            {
                Ok((new_url, new_key)) => {
                    url = new_url;
                    key = new_key;
                    log::info!("任务 {} 获取到新下载链接", ctx.task_id);
                }
                Err(e) => {
                    // 将具体错误信息发送到前端，便于用户了解失败原因
                    log::error!("任务 {} 最终获取下载链接失败: {}", ctx.task_id, e);
                    progress::emit_error(
                        &app_handle,
                        &ctx.task_id,
                        &format!("获取下载链接失败: {}", e),
                    );
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
            match open_download_file(
                &app_handle,
                &ctx.task_id,
                &download_dir,
                is_saf,
                saf_folder_uri.as_deref(),
                &mut downloaded,
                &mut saf_file_uri,
            )
            .await
            {
                Some(f) => {
                    file = Some(f);

                    // 更新 final_path：SAF 模式为 URI，普通模式为普通路径
                    if is_saf {
                        if let Some(uri) = saf_file_uri.clone() {
                            *controller.final_path.lock().await = Some(uri);
                        }
                    } else {
                        *controller.final_path.lock().await = Some(download_dir.clone());
                    }
                }
                None => break 'download,
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
                        // 发送具体请求错误（如连接超时、TLS 错误等）到前端
                        log::error!("任务 {} 最终下载请求失败: {}", ctx.task_id, e);
                        progress::emit_error(
                            &app_handle,
                            &ctx.task_id,
                            &format!("下载请求失败: {}", e),
                        );
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
                    .and_then(|s| s.split('/').next_back().and_then(|n| n.parse::<u64>().ok()))
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
                    // 刷新缓冲区失败时告知具体原因（如磁盘 I/O 错误）
                    log::error!("刷新文件缓冲区失败: {}", e);
                    progress::emit_error(
                        &app_handle,
                        &ctx.task_id,
                        &format!("刷新文件缓冲区失败: {}", e),
                    );
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
                // 将 StatusCode 格式化后作为错误消息的一部分，让用户知道具体的 HTTP 状态码
                log::error!("任务 {} 服务器错误: {}", ctx.task_id, status);
                progress::emit_error(
                    &app_handle,
                    &ctx.task_id,
                    &format!("服务器错误: HTTP {}", status),
                );
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
                        // 流读取失败（如连接重置、超时）时告知用户具体原因
                        progress::emit_error(
                            &app_handle,
                            &ctx.task_id,
                            &format!("读取流错误: {}", e),
                        );
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
                    // 写入文件失败时提供具体错误
                    log::error!("写入文件错误: {}", e);
                    progress::emit_error(
                        &app_handle,
                        &ctx.task_id,
                        &format!("写入文件失败: {}", e),
                    );
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
                        // 最终刷新失败时提供具体错误
                        log::error!("刷新文件缓冲区失败: {}", e);
                        progress::emit_error(
                            &app_handle,
                            &ctx.task_id,
                            &format!("刷新文件缓冲区失败: {}", e),
                        );
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
        // 文件下载完成，发送事件，前端进入 processing 状态
        progress::emit_file_complete(&app_handle, &ctx.task_id);

        // 获取歌词（仅当需要写入 metadata 或单独下载 lrc 时才请求）
        let lyric_resp = if write_metadata_enabled || download_lrc_enabled {
            match lyrics::get_lyric_by_id_inner(ctx.platform, ctx.song_id).await {
                Ok(resp) => Some(resp),
                Err(e) => {
                    log::warn!("获取歌词失败: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // 修复酷我封面缺失问题：酷我搜索阶段不返回封面，因此封面 URL 为空。
        // 若用户开启了 metadata 写入且平台为酷我，且当前 cover_url 为空，则主动调用酷我封面接口获取。
        // 获取失败时保留原值（空字符串），不阻断歌词写入。
        let cover_url_to_use = if write_metadata_enabled
            && ctx.song_info.cover_url.is_empty()
            && matches!(ctx.platform, Platform::Kuwo)
        {
            match crate::platforms::kuwo::cover::fetch_cover(ctx.song_id).await {
                Ok(url) => {
                    log::info!("任务 {} 成功获取酷我封面 URL: {}", ctx.task_id, url);
                    url
                }
                Err(e) => {
                    log::warn!(
                        "任务 {} 获取酷我封面失败，将继续使用空封面: {}",
                        ctx.task_id,
                        e
                    );
                    ctx.song_info.cover_url.clone()
                }
            }
        } else {
            ctx.song_info.cover_url.clone()
        };

        // 写入音频文件 metadata（歌词/封面），错误处理在函数内部完成
        if write_metadata_enabled {
            write_metadata(
                &app_handle,
                &ctx.task_id,
                &download_dir,
                is_saf,
                saf_file_uri.clone(),
                &cover_url_to_use,
                lyric_resp.clone(),
            )
            .await;
        }

        // 单独下载 LRC 歌词文件（仅当有普通歌词且开关开启）
        if download_lrc_enabled {
            if let Some(lrc) = lyric_resp
                .as_ref()
                .and_then(|r| r.lrc.as_ref())
                .filter(|s| !s.trim().is_empty())
            {
                let lrc_path = write_lrc_file(
                    &app_handle,
                    lrc,
                    &download_dir,
                    is_saf,
                    saf_folder_uri.clone(),
                )
                .await;
                if let Some(path) = lrc_path {
                    *controller.lrc_final_path.lock().await = Some(path);
                }
            } else {
                log::info!("无普通歌词，跳过 LRC 文件创建");
            }
        }

        // 全部处理完成，发送下载完成事件
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

    completed_ok
}

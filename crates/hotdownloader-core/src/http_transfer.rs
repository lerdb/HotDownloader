use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};

use crate::decryption::{self, DecryptContext};
use crate::engine::TaskController;
use crate::ports::DownloadProgressSink;

/// 下载专用客户端不设置总超时，大文件只限制连接和单次读取等待时间。
static DOWNLOAD_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn download_client() -> &'static reqwest::Client {
    DOWNLOAD_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("HotDownloader/1.0")
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(300))
            .build()
            .expect("Failed to create download HTTP client")
    })
}

/// 使用当前已写入字节数请求 HTTP Range；临时请求错误最多尝试三次。
/// 返回原始响应，以便文件适配器处理普通路径或 Android SAF 文件。
pub async fn request_download_response(
    url: &str,
    downloaded: u64,
    task_id: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempt = 0;
    loop {
        let mut request = download_client()
            .get(url)
            .header("Referer", "https://y.qq.com");
        if downloaded > 0 {
            request = request.header(RANGE, format!("bytes={downloaded}-"));
        }

        match request.send().await {
            Ok(response) => return Ok(response),
            Err(error) => {
                attempt += 1;
                log::warn!("任务 {task_id} 下载请求失败 (尝试 {attempt}/3): {error}");
                if is_retryable_network_error(&error) && attempt < 3 {
                    // 两次等待依次为 1 秒、2 秒，第三次失败直接交给 worker 上报。
                    tokio::time::sleep(Duration::from_secs(1 << (attempt - 1))).await;
                } else {
                    return Err(error);
                }
            }
        }
    }
}

/// reqwest 错误分类留在传输核心，平台接口返回的字符串错误另由链接模块判断。
pub fn is_retryable_network_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || (error.is_request() && !error.is_body())
}

/// worker 根据响应决定是否重新打开文件、继续流写入或上报错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseAction {
    Stream { total: u64 },
    RestartFromBeginning { total: u64 },
    InvalidRange,
    AlreadyComplete,
    LinkExpired,
    HttpError(u16),
}

/// 一次 HTTP 响应读取结束后，worker 据此决定完成、等待恢复或重新获取链接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOutcome {
    Completed,
    Paused,
    Canceled,
    Retry,
    Failed,
}

/// 把响应流按文件绝对偏移解密并写入目标。文件可以是普通文件，也可以是 SAF 适配器打开的文件。
/// 文件的创建和重新打开仍由上层负责，因为 Android SAF 需要平台授权及 URI。
pub async fn write_response_stream<W: Write + Send>(
    response: reqwest::Response,
    writer: &mut W,
    downloaded: &mut u64,
    total: u64,
    decrypt_context: &DecryptContext,
    controller: &TaskController,
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
    stream_retries: &mut u32,
) -> StreamOutcome {
    let mut stream = response.bytes_stream();
    let mut last_report = Instant::now();
    let mut last_downloaded = *downloaded;
    let mut speed_samples = VecDeque::with_capacity(5);

    loop {
        if controller.cancel_token.is_cancelled() {
            return StreamOutcome::Canceled;
        }
        if controller.pause_flag.load(Ordering::SeqCst) {
            return StreamOutcome::Paused;
        }

        // 读取网络数据期间也响应取消，不让移除任务长期等待读取超时。
        let chunk_result = tokio::select! {
            _ = controller.cancel_token.cancelled() => return StreamOutcome::Canceled,
            chunk = stream.next() => chunk,
        };
        let chunk = match chunk_result {
            Some(Ok(bytes)) => bytes,
            Some(Err(error)) => {
                log::error!("任务 {task_id} 读取流错误: {error}");
                return retry_or_fail(
                    stream_retries,
                    progress_sink,
                    task_id,
                    &format!("读取流错误: {error}"),
                );
            }
            None => {
                // 正常 EOF 只有在文件大小未知或达到预期大小时才算完成。
                // 已知大小却提前结束时重新请求 Range，防止任务永远留在下载中。
                if total > 0 && *downloaded < total {
                    return retry_or_fail(
                        stream_retries,
                        progress_sink,
                        task_id,
                        "下载流提前结束，请重试",
                    );
                }
                return flush_completed(writer, progress_sink, task_id);
            }
        };

        let mut chunk_data = chunk.to_vec();
        let chunk_len = chunk_data.len() as u64;
        decryption::decrypt_chunk(decrypt_context, &mut chunk_data, *downloaded);
        if let Err(error) = writer.write_all(&chunk_data) {
            log::error!("写入文件错误: {error}");
            progress_sink.error(task_id, &format!("写入文件失败: {error}"));
            return StreamOutcome::Failed;
        }
        *downloaded += chunk_len;

        let now = Instant::now();
        let elapsed = now - last_report;
        if elapsed >= Duration::from_millis(500) {
            let instant_speed =
                ((*downloaded - last_downloaded) as f64 / elapsed.as_secs_f64()) as u64;
            speed_samples.push_back(instant_speed);
            if speed_samples.len() > 5 {
                speed_samples.pop_front();
            }
            let average_speed = speed_samples.iter().sum::<u64>() / speed_samples.len() as u64;
            progress_sink.progress(task_id, *downloaded, total, average_speed);
            last_report = now;
            last_downloaded = *downloaded;
        }

        if total > 0 && *downloaded >= total {
            return flush_completed(writer, progress_sink, task_id);
        }
    }
}

/// 流错误累计最多重试两次；第三次必须进入明确的错误状态。
fn retry_or_fail(
    stream_retries: &mut u32,
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
    message: &str,
) -> StreamOutcome {
    if *stream_retries < 2 {
        *stream_retries += 1;
        StreamOutcome::Retry
    } else {
        progress_sink.error(task_id, message);
        StreamOutcome::Failed
    }
}

/// 只有缓冲区真正写入成功后，才能将传输视为完成。
fn flush_completed<W: Write>(
    writer: &mut W,
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
) -> StreamOutcome {
    if let Err(error) = writer.flush() {
        log::error!("刷新文件缓冲区失败: {error}");
        progress_sink.error(task_id, &format!("刷新文件缓冲区失败: {error}"));
        StreamOutcome::Failed
    } else {
        StreamOutcome::Completed
    }
}

/// 从真实 HTTP 响应提取状态与头部，判断续传是否安全。
pub fn classify_http_response(
    response: &reqwest::Response,
    downloaded: u64,
    fallback_size: u64,
) -> ResponseAction {
    let content_range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok());
    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok());
    classify_response(
        response.status().as_u16(),
        content_range,
        content_length,
        downloaded,
        fallback_size,
    )
}

/// 与运行时无关的 HTTP 响应规则。原有 416 行为视为文件已下载完成。
pub fn classify_response(
    status: u16,
    content_range: Option<&str>,
    content_length: Option<&str>,
    downloaded: u64,
    fallback_size: u64,
) -> ResponseAction {
    // Content-Range 的末尾为整个文件大小；缺失或无效时退回 Content-Length/任务快照。
    let reported_size = if let Some(range) = content_range {
        range
            .rsplit('/')
            .next()
            .and_then(|value| value.parse().ok())
    } else {
        content_length.and_then(|value| value.parse().ok())
    };
    let total = reported_size
        .filter(|size| *size > 0)
        .unwrap_or(fallback_size);

    // CDN 忽略 Range 时返回完整 200，继续追加会把两份音频拼在一起。
    if downloaded > 0 && status == 200 {
        return ResponseAction::RestartFromBeginning { total };
    }

    if downloaded > 0 && status == 206 {
        let range_start = content_range
            .and_then(|value| value.strip_prefix("bytes "))
            .and_then(|value| value.split('-').next())
            .and_then(|value| value.parse::<u64>().ok());
        if range_start != Some(downloaded) {
            return ResponseAction::InvalidRange;
        }
    }

    if status == 416 {
        return ResponseAction::AlreadyComplete;
    }
    if matches!(status, 403 | 404 | 410) {
        return ResponseAction::LinkExpired;
    }
    if (400..600).contains(&status) {
        return ResponseAction::HttpError(status);
    }

    ResponseAction::Stream { total }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{classify_response, retry_or_fail, ResponseAction, StreamOutcome};
    use crate::ports::DownloadProgressSink;

    #[derive(Default)]
    struct RecordingProgressSink {
        errors: Mutex<Vec<String>>,
    }

    impl DownloadProgressSink for RecordingProgressSink {
        fn progress(&self, _task_id: &str, _downloaded: u64, _total: u64, _speed: u64) {}
        fn file_complete(&self, _task_id: &str) {}
        fn completed(&self, _task_id: &str, _final_path: &str, _saf_folder_uri: Option<String>) {}
        fn error(&self, _task_id: &str, message: &str) {
            self.errors.lock().unwrap().push(message.to_string());
        }
        fn link_expired(&self, _task_id: &str, _current_offset: u64) {}
        fn metadata_error(&self, _task_id: &str, _message: &str) {}
    }

    #[test]
    fn ignored_range_requires_reopening_file_from_zero() {
        assert_eq!(
            classify_response(200, None, Some("100"), 30, 100),
            ResponseAction::RestartFromBeginning { total: 100 }
        );
    }

    #[test]
    fn partial_response_must_start_at_requested_offset() {
        assert_eq!(
            classify_response(206, Some("bytes 10-99/100"), None, 30, 100),
            ResponseAction::InvalidRange
        );
        assert_eq!(
            classify_response(206, Some("bytes 30-99/100"), Some("70"), 30, 70),
            ResponseAction::Stream { total: 100 }
        );
    }

    #[test]
    fn expired_link_and_unknown_size_follow_existing_contract() {
        assert_eq!(
            classify_response(403, None, None, 12, 100),
            ResponseAction::LinkExpired
        );
        assert_eq!(
            classify_response(416, None, None, 100, 100),
            ResponseAction::AlreadyComplete
        );
        assert_eq!(
            classify_response(200, None, None, 0, 500),
            ResponseAction::Stream { total: 500 }
        );
    }

    #[test]
    fn repeated_stream_failure_reports_terminal_error() {
        let sink = RecordingProgressSink::default();
        let mut retries = 0;

        assert_eq!(
            retry_or_fail(&mut retries, &sink, "task", "断流"),
            StreamOutcome::Retry
        );
        assert_eq!(
            retry_or_fail(&mut retries, &sink, "task", "断流"),
            StreamOutcome::Retry
        );
        assert_eq!(
            retry_or_fail(&mut retries, &sink, "task", "断流"),
            StreamOutcome::Failed
        );
        assert_eq!(retries, 2);
        assert_eq!(sink.errors.lock().unwrap().as_slice(), ["断流"]);
    }
}

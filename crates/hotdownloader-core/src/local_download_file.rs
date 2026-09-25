use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter};
use std::path::Path;

use futures_util::future::BoxFuture;

use crate::ports::{DownloadFileOpener, DownloadProgressSink, FileOpenRequest, OpenedDownloadFile};

/// 普通文件下载使用与 SAF 写入相同的 64 KB 缓冲区大小。
const FILE_BUFFER_CAPACITY: usize = 64 * 1024;

/// 打开失败时同时保留底层 I/O 错误和面向用户的原有提示文案。
#[derive(Debug)]
pub struct LocalFileOpenError {
    pub operation: &'static str,
    pub user_message: &'static str,
    pub source: io::Error,
}

impl std::fmt::Display for LocalFileOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.source)
    }
}

/// 创建目标文件的父目录。默认目录的选择和校验由 `DownloadConfig` 完成。
pub fn ensure_parent_directory(file_path: &str) -> io::Result<()> {
    let parent = Path::new(file_path).parent().unwrap_or(Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// 打开普通文件并校验续传偏移。文件短于已下载偏移时必须清空重下，避免音频错位。
/// 保留原有的追加行为：文件比偏移大时仍从文件尾部追加，由上层处理 HTTP Range。
pub fn open_local_download_file(
    file_path: &str,
    downloaded: &mut u64,
) -> Result<BufWriter<File>, LocalFileOpenError> {
    if *downloaded == 0 {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
            .map_err(|source| LocalFileOpenError {
                operation: "文件创建失败",
                user_message: "无法创建下载文件，请检查磁盘空间或下载目录的创建/写入权限",
                source,
            })?;
        return Ok(BufWriter::with_capacity(FILE_BUFFER_CAPACITY, file));
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .map_err(|source| LocalFileOpenError {
            operation: "文件打开失败",
            user_message: "文件访问失败",
            source,
        })?;

    // 元数据读取失败或文件被截断时，保守地从头开始下载。
    let should_reset = file
        .metadata()
        .map(|metadata| metadata.len() < *downloaded)
        .unwrap_or(true);
    if !should_reset {
        return Ok(BufWriter::with_capacity(FILE_BUFFER_CAPACITY, file));
    }

    // Windows 不允许在旧句柄仍占用文件时重新以截断模式打开。
    drop(file);
    let reset_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)
        .map_err(|source| LocalFileOpenError {
            operation: "文件重置失败",
            user_message: "文件异常，请重试",
            source,
        })?;
    *downloaded = 0;
    Ok(BufWriter::with_capacity(FILE_BUFFER_CAPACITY, reset_file))
}

/// 独立 Rust 运行时可直接使用的普通文件适配器。
pub struct LocalDownloadFileOpener;

impl DownloadFileOpener for LocalDownloadFileOpener {
    fn open<'a>(
        &'a self,
        request: FileOpenRequest<'a>,
        progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Option<OpenedDownloadFile>> {
        Box::pin(async move {
            if request.is_saf {
                progress_sink.error(request.task_id, "普通文件适配器不支持 SAF 路径");
                return None;
            }
            let mut downloaded = request.downloaded;
            match open_local_download_file(request.file_path, &mut downloaded) {
                Ok(writer) => Some(OpenedDownloadFile {
                    writer,
                    downloaded,
                    saf_file_uri: None,
                }),
                Err(error) => {
                    log::error!("{error}");
                    progress_sink.error(request.task_id, error.user_message);
                    None
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::open_local_download_file;

    #[test]
    fn truncated_partial_file_resets_offset_and_contents() {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-file-open-{}.part",
            rand::random::<u64>()
        ));
        std::fs::write(&path, b"old").unwrap();

        let mut downloaded = 10;
        let mut writer = open_local_download_file(path.to_str().unwrap(), &mut downloaded).unwrap();
        writer.write_all(b"new").unwrap();
        writer.flush().unwrap();
        drop(writer);

        assert_eq!(downloaded, 0);
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        std::fs::remove_file(path).unwrap();
    }
}

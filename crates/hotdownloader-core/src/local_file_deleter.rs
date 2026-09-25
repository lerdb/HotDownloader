use std::io::ErrorKind;

use futures_util::future::BoxFuture;

use super::ports::FileDeleter;

/// 普通文件系统删除实现；Docker 运行时可直接使用。
pub struct LocalFileDeleter;

impl FileDeleter for LocalFileDeleter {
    fn remove<'a>(&'a self, path: &'a str) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                // 任务记录可能还在，文件已被用户手动删除时不应让清理失败。
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("删除文件失败: {error}")),
            }
        })
    }
}

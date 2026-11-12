use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::Client;

use crate::platforms::qqmusic::credentials::{FileQqCredentialSource, QqCredentialSource};
use crate::platforms::Platform;

/// 平台链接和凭据由运行时提供；核心只决定何时重新请求链接。
pub trait DownloadLinkProvider: Send + Sync {
    fn queue_control(&self) -> Option<Arc<super::queue::QueueControl>> {
        None
    }
    fn fetch<'a>(
        &'a self,
        platform: Platform,
        song_mid: &'a str,
        filename: &'a str,
    ) -> BoxFuture<'a, Result<(String, String), String>>;
}

/// 平台共用的 HTTP 链接提供器。运行时只注入凭据来源；下载任务不接触令牌。
pub struct PlatformDownloadLinkProvider {
    queue: Option<Arc<super::queue::QueueControl>>,
    client: Client,
    qq_credentials: Arc<dyn QqCredentialSource>,
    kugou_store: Option<Arc<crate::platforms::kugou::login::Store>>,
    netease_store: Option<Arc<crate::platforms::netease::login::Store>>,
}

impl PlatformDownloadLinkProvider {
    pub fn new(qq_credentials: Arc<dyn QqCredentialSource>) -> Self {
        let client = link_client();
        Self {
            queue: None,
            client,
            qq_credentials,
            kugou_store: None,
            netease_store: None,
        }
    }

    /// 独立进程入口只需提供持久化凭据文件路径；凭据会在每次请求时读取并按需刷新。
    pub fn from_credentials_file(path: impl Into<PathBuf>) -> Self {
        let client = link_client();
        let qq_credentials = Arc::new(FileQqCredentialSource::new(path, client.clone()));
        Self {
            queue: None,
            client,
            qq_credentials,
            kugou_store: None,
            netease_store: None,
        }
    }

    /// 允许运行时复用已有 HTTP 客户端，也便于测试使用本地服务。
    pub fn with_client(client: Client, qq_credentials: Arc<dyn QqCredentialSource>) -> Self {
        Self {
            queue: None,
            client,
            qq_credentials,
            kugou_store: None,
            netease_store: None,
        }
    }

    pub fn with_kugou_store(mut self, store: Arc<crate::platforms::kugou::login::Store>) -> Self {
        self.kugou_store = Some(store);
        self
    }

    pub fn with_queue_control(mut self, queue: Arc<super::queue::QueueControl>) -> Self {
        self.queue = Some(queue);
        self
    }

    pub fn with_netease_store(
        mut self,
        store: Arc<crate::platforms::netease::login::Store>,
    ) -> Self {
        self.netease_store = Some(store);
        self
    }
}

fn link_client() -> Client {
    Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/86.0.4240.198 Safari/537.36")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create link HTTP client")
}

impl DownloadLinkProvider for PlatformDownloadLinkProvider {
    fn queue_control(&self) -> Option<Arc<super::queue::QueueControl>> {
        self.queue.clone()
    }
    fn fetch<'a>(
        &'a self,
        platform: Platform,
        song_mid: &'a str,
        filename: &'a str,
    ) -> BoxFuture<'a, Result<(String, String), String>> {
        Box::pin(async move {
            match platform {
                Platform::Netease => {
                    let cookie = self
                        .netease_store
                        .as_ref()
                        .map(|s| s.cookie())
                        .transpose()?
                        .unwrap_or_default();
                    crate::platforms::netease::link::get_download_link(song_mid, filename, &cookie)
                        .await
                }
                Platform::Kugou => {
                    let params = self
                        .kugou_store
                        .as_ref()
                        .map(|s| s.request_params())
                        .transpose()?
                        .unwrap_or_else(|| serde_json::json!({}));
                    crate::platforms::kugou::link::get_download_link_with_credentials(
                        &self.client,
                        song_mid,
                        filename,
                        params,
                    )
                    .await
                }
                Platform::QqMusic => {
                    let credentials = self.qq_credentials.current().await?;
                    crate::platforms::qqmusic::link::fetch_vkey_link(
                        &self.client,
                        song_mid,
                        filename,
                        credentials.as_ref(),
                    )
                    .await
                }
                // 酷我当前接口不使用 QQ 登录态，不能因 QQ 凭据文件出错而阻止酷我任务。
                Platform::Kuwo => {
                    crate::platforms::kuwo::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                    )
                    .await
                }
            }
        })
    }
}

/// 临时网络错误最多尝试三次，平台明确拒绝时立即返回错误。
pub async fn fetch_download_link_with_retry(
    provider: &dyn DownloadLinkProvider,
    song_mid: &str,
    filename: &str,
    task_id: &str,
    platform: Platform,
) -> Result<(String, String), String> {
    let mut last_error = String::new();

    for attempt in 0..3 {
        let queue = provider
            .queue_control()
            .filter(|_| platform == Platform::QqMusic);
        let result = if let Some(queue) = queue {
            queue
                .fetch(song_mid, || provider.fetch(platform, song_mid, filename))
                .await
        } else {
            provider.fetch(platform, song_mid, filename).await
        };
        match result {
            Ok(link) => return Ok(link),
            Err(error) => {
                if !is_retryable_link_error(&error) {
                    // 登录态失效和平台拒绝属于确定性错误，重复请求只会延迟任务失败。
                    log::warn!("任务 {task_id} 获取下载链接失败: {error}");
                    return Err(error);
                }

                log::warn!(
                    "任务 {task_id} 获取下载链接失败 (尝试 {}/3): {error}",
                    attempt + 1
                );
                last_error = error;
                if attempt < 2 {
                    // 首次和第二次失败后分别等待 1 秒、2 秒；第三次直接返回最终错误。
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                }
            }
        }
    }

    Err(last_error)
}

/// 错误分类沿用现有平台接口的中文错误前缀，保持客户端原有重试行为。
pub fn is_retryable_link_error(error: &str) -> bool {
    error.starts_with("网络错误")
        || error.starts_with("取链接口 HTTP 错误")
            && ["408", "500", "502", "503", "504"]
                .iter()
                .any(|code| error.contains(code))
        || error.starts_with("读取响应失败")
        || error.starts_with("解析响应失败")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        fetch_download_link_with_retry, DownloadLinkProvider, PlatformDownloadLinkProvider,
    };
    use crate::platforms::Platform;
    use futures_util::future::BoxFuture;

    struct RejectedLink {
        attempts: AtomicUsize,
    }

    struct RecoveringLink {
        attempts: AtomicUsize,
    }

    impl DownloadLinkProvider for RejectedLink {
        fn fetch<'a>(
            &'a self,
            _platform: Platform,
            _song_mid: &'a str,
            _filename: &'a str,
        ) -> BoxFuture<'a, Result<(String, String), String>> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err("平台拒绝: 104003".to_string()) })
        }
    }

    impl DownloadLinkProvider for RecoveringLink {
        fn fetch<'a>(
            &'a self,
            _platform: Platform,
            _song_mid: &'a str,
            _filename: &'a str,
        ) -> BoxFuture<'a, Result<(String, String), String>> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if attempt == 0 {
                    Err("网络错误: 连接超时".to_string())
                } else {
                    Ok(("https://example.test/audio".to_string(), String::new()))
                }
            })
        }
    }

    #[tokio::test]
    async fn platform_rejection_is_not_retried() {
        let provider = RejectedLink {
            attempts: AtomicUsize::new(0),
        };
        let result = fetch_download_link_with_retry(
            &provider,
            "song",
            "file.mp3",
            "task",
            Platform::QqMusic,
        )
        .await;

        assert_eq!(result.unwrap_err(), "平台拒绝: 104003");
        assert_eq!(provider.attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn temporary_network_error_retries_and_returns_link() {
        let provider = RecoveringLink {
            attempts: AtomicUsize::new(0),
        };
        let result = fetch_download_link_with_retry(
            &provider,
            "song",
            "file.mp3",
            "task",
            Platform::QqMusic,
        )
        .await;

        assert_eq!(
            result.unwrap().0,
            "https://example.test/audio",
            "第二次请求成功后应立即返回真实链接"
        );
        assert_eq!(provider.attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn kuwo_link_does_not_read_qq_credential_file() {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-invalid-qq-{:x}.json",
            rand::random::<u64>()
        ));
        tokio::fs::write(&path, "{").await.unwrap();
        let provider = PlatformDownloadLinkProvider::from_credentials_file(&path);

        // 非法酷我 ID 在请求前失败；若错误来自 QQ 文件，说明平台隔离失效。
        let error = provider
            .fetch(Platform::Kuwo, "invalid-id", "320.mp3")
            .await
            .unwrap_err();
        assert!(error.contains("无效的歌曲 ID"));
        let error = provider
            .fetch(Platform::Kugou, "invalid-hash", "invalid.mp3")
            .await
            .unwrap_err();
        assert!(error.contains("无效的酷狗音源标识"));
        tokio::fs::remove_file(path).await.unwrap();
    }
}

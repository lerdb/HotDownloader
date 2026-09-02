use once_cell::sync::Lazy;

/// 全局复用 HTTP 客户端，启用连接池、超时等
pub(crate) static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/86.0.4240.198 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30)) // 整体请求超时
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
});

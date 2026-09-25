//! 与运行时无关的音乐平台查询和歌词实现。下载链接入口在 `download_link`。

use once_cell::sync::Lazy;
use rand::Rng;

pub mod kuwo;
pub mod lyric;
pub mod qqmusic;

/// 平台查询复用短超时 HTTP 客户端；与大文件流下载客户端分开。
pub(crate) static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/86.0.4240.198 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to create platform HTTP client")
});

/// QQ 查询请求使用原有 32 位大写字母与数字 GUID 格式。
pub(crate) fn get_guid() -> String {
    const CHARSET: &[u8] = b"ABCDEF1234567890";
    let mut rng = rand::rng();
    (0..32)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}

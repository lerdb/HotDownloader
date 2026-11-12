//! 下载封面按平台选择原图或高清地址，供音频标签及独立图片文件共用。

use lofty::picture::Picture;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{Client, Url};

use super::config::CoverQuality;
use crate::platforms::Platform;

static QQ_PICTURE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(/music/photo_new/T00[12])(?:R\d+x\d+)?(M000[^/]+\.jpg)$").unwrap());
static KUWO_PICTURE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(/star/(?:albumcover|starheads)/)\d+/(.+)$").unwrap());

fn candidate_urls(platform: Platform, source: &str, quality: CoverQuality) -> Vec<String> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    if quality != CoverQuality::Source {
        if let Ok(mut url) = Url::parse(source) {
            let host = url.host_str().unwrap_or("").to_string();
            if platform == Platform::Netease
                && (host == "music.126.net" || host.ends_with(".music.126.net"))
            {
                let pairs: Vec<_> = url
                    .query_pairs()
                    .filter(|(key, _)| key != "param")
                    .map(|(key, value)| (key.into_owned(), value.into_owned()))
                    .collect();
                url.set_query(None);
                if !pairs.is_empty() {
                    url.query_pairs_mut().extend_pairs(pairs);
                }
                if quality == CoverQuality::High {
                    url.query_pairs_mut().append_pair("param", "500y500");
                }
                candidates.push(url.to_string());
            }
            let sizes: &[&str] = match quality {
                CoverQuality::Original => &["", "800", "500"],
                CoverQuality::High => &["500"],
                CoverQuality::Source => &[],
            };
            let paths: Vec<String> = match platform {
                Platform::QqMusic if host == "y.gtimg.cn" => QQ_PICTURE
                    .captures(url.path())
                    .map(|parts| {
                        sizes
                            .iter()
                            .map(|size| {
                                let resize = if size.is_empty() {
                                    String::new()
                                } else {
                                    format!("R{size}x{size}")
                                };
                                format!("{}{resize}{}", &parts[1], &parts[2])
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                Platform::Kuwo if host == "kuwo.cn" || host.ends_with(".kuwo.cn") => {
                    let sizes = if quality == CoverQuality::Original {
                        &["0", "1000", "500"][..]
                    } else {
                        &["500"][..]
                    };
                    KUWO_PICTURE
                        .captures(url.path())
                        .map(|parts| {
                            sizes
                                .iter()
                                .map(|size| format!("{}{size}/{}", &parts[1], &parts[2]))
                                .collect()
                        })
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            };
            for path in paths {
                url.set_path(&path);
                let candidate = url.to_string();
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
    }
    if !candidates.iter().any(|candidate| candidate == source) {
        candidates.push(source.to_string());
    }
    candidates
}

async fn fetch_candidates(client: &Client, candidates: &[String]) -> Option<Vec<u8>> {
    for url in candidates {
        let result = async {
            let response = client.get(url).send().await?.error_for_status()?;
            response.bytes().await
        }
        .await;
        match result {
            Ok(bytes) => {
                // 某些 CDN 以 200 返回错误页；不把 HTML 或空响应写成封面。
                if let Ok(picture) = Picture::from_reader(&mut bytes.as_ref()) {
                    return Some(picture.into_data());
                }
                log::debug!("封面响应不是有效图片，尝试下一个地址");
            }
            Err(error) => log::debug!("获取封面失败，尝试下一个地址: {error}"),
        }
    }
    None
}

pub(super) async fn fetch_cover_bytes(
    platform: Platform,
    source: &str,
    quality: CoverQuality,
) -> Option<Vec<u8>> {
    fetch_candidates(
        &crate::platforms::CLIENT,
        &candidate_urls(platform, source, quality),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netease_cover_sizes_keep_the_source_as_fallback() {
        let source = "https://p1.music.126.net/cover.jpg?param=200y200";
        assert_eq!(
            candidate_urls(Platform::Netease, source, CoverQuality::Original),
            ["https://p1.music.126.net/cover.jpg", source]
        );
        assert_eq!(
            candidate_urls(Platform::Netease, source, CoverQuality::High),
            ["https://p1.music.126.net/cover.jpg?param=500y500", source]
        );
    }

    #[test]
    fn qq_original_uses_unscaled_artwork_then_supported_sizes() {
        for kind in ["T001", "T002"] {
            let base = format!("https://y.gtimg.cn/music/photo_new/{kind}");
            let source = format!("{base}R300x300M000album.jpg?max_age=2592000");
            assert_eq!(
                candidate_urls(Platform::QqMusic, &source, CoverQuality::Original),
                [
                    format!("{base}M000album.jpg?max_age=2592000"),
                    format!("{base}R800x800M000album.jpg?max_age=2592000"),
                    format!("{base}R500x500M000album.jpg?max_age=2592000"),
                    source,
                ]
            );
        }
    }

    #[test]
    fn kuwo_upgrades_album_and_artist_pictures_and_keeps_source_fallback() {
        for directory in ["albumcover", "starheads"] {
            let base = format!("https://img4.kuwo.cn/star/{directory}");
            let source = format!("{base}/300/55/84/picture.jpg");
            assert_eq!(
                candidate_urls(Platform::Kuwo, &source, CoverQuality::Original),
                [
                    format!("{base}/0/55/84/picture.jpg"),
                    format!("{base}/1000/55/84/picture.jpg"),
                    format!("{base}/500/55/84/picture.jpg"),
                    source,
                ]
            );
        }
    }

    #[test]
    fn source_unknown_hosts_and_unknown_paths_are_preserved() {
        for (platform, source) in [
            (Platform::QqMusic, "https://y.gtimg.cn/other/300/image.jpg"),
            (
                Platform::Kuwo,
                "https://kuwo.cn.example.com/star/albumcover/300/a.jpg",
            ),
            (
                Platform::Kuwo,
                "https://example.com/star/albumcover/300/a.jpg",
            ),
        ] {
            assert_eq!(
                candidate_urls(platform, source, CoverQuality::Original),
                [source]
            );
        }
        let source = "https://y.gtimg.cn/music/photo_new/T002R300x300M000album.jpg";
        assert_eq!(
            candidate_urls(Platform::QqMusic, source, CoverQuality::Source),
            [source]
        );
        assert!(candidate_urls(Platform::Kuwo, "", CoverQuality::Original).is_empty());
    }

    #[test]
    fn high_quality_uses_500_without_duplicate_requests() {
        for (platform, source) in [
            (
                Platform::QqMusic,
                "https://y.gtimg.cn/music/photo_new/T002R500x500M000album.jpg",
            ),
            (
                Platform::Kuwo,
                "https://img4.kuwo.cn/star/albumcover/500/a.jpg",
            ),
        ] {
            assert_eq!(
                candidate_urls(platform, source, CoverQuality::High),
                [source]
            );
        }
        let base = "https://y.gtimg.cn/music/photo_new/T002";
        let source = format!("{base}R300x300M000album.jpg");
        assert_eq!(
            candidate_urls(Platform::QqMusic, &source, CoverQuality::High),
            [format!("{base}R500x500M000album.jpg"), source]
        );
    }

    #[tokio::test]
    async fn failed_and_invalid_images_fall_back_and_stop_at_first_picture() {
        use std::io::{Read, Write};
        use std::time::{Duration, Instant};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let image = b"\x89PNG\r\n\x1a\n";
        let server = std::thread::spawn(move || {
            for (path, status, body) in [
                ("/original", "404 Not Found", &b"missing"[..]),
                ("/800", "200 OK", &b"<html>error</html>"[..]),
                ("/500", "200 OK", &image[..]),
            ] {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("local HTTP accept failed: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut bytes = [0; 1024];
                    let count = stream.read(&mut bytes).unwrap();
                    assert!(count > 0 && request.len() < 16 * 1024);
                    request.extend_from_slice(&bytes[..count]);
                }
                assert!(String::from_utf8_lossy(&request).starts_with(&format!("GET {path} ")));
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let candidates = ["original", "800", "500", "source"].map(|path| format!("{base}/{path}"));
        assert_eq!(
            fetch_candidates(&client, &candidates).await.as_deref(),
            Some(image.as_slice())
        );
        server.join().unwrap();
        assert!(fetch_candidates(&client, &candidates).await.is_none());
    }
}

//! 普通文件系统的歌词、封面与音频标签写入，以及完整下载收尾实现。

use std::path::Path;

use futures_util::future::BoxFuture;
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::tag::{ItemKey, Tag, TagType};

use crate::download::context::TaskContext;
use crate::download::ports::{DownloadPostprocessor, DownloadProgressSink, PostprocessRequest};
use crate::download::postprocess::{
    cover_file_name, lrc_file_name, prepare_assets, PostprocessAssets,
};
use crate::platforms::lyric::LyricData;

/// 将普通 LRC 写在音频旁边；写入失败返回 None，由收尾适配器报告错误。
pub fn write_local_lrc(audio_path: &str, lyric: &LyricData) -> Option<String> {
    let content = lyric
        .lrc
        .as_deref()
        .filter(|value| !value.trim().is_empty())?;
    write_local_sidecar(audio_path, &lrc_file_name(audio_path), content.as_bytes())
}

/// 将封面原始图片保存在音频旁边，使用实际图片格式的扩展名。
pub fn write_local_cover(audio_path: &str, cover_bytes: &[u8]) -> Option<String> {
    let Some(name) = cover_file_name(audio_path, cover_bytes) else {
        log::warn!("封面图片格式无效，跳过封面文件创建");
        return None;
    };
    write_local_sidecar(audio_path, &name, cover_bytes)
}

fn write_local_sidecar(audio_path: &str, file_name: &str, content: &[u8]) -> Option<String> {
    let parent = Path::new(audio_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = parent.join(file_name);
    match std::fs::write(&path, content) {
        Ok(()) => {
            log::info!("附属文件已保存: {}", path.display());
            Some(path.to_string_lossy().to_string())
        }
        Err(error) => {
            log::warn!("写入附属文件失败 {}: {error}", path.display());
            None
        }
    }
}

/// 在同目录临时副本中写标签，重新读取核对后替换原文件。
pub fn write_audio_metadata(
    path: &Path,
    context: &TaskContext,
    assets: &PostprocessAssets,
) -> Result<bool, String> {
    let lyric_text = assets.lyric.as_ref().and_then(|lyric| {
        lyric
            .elrc
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                lyric
                    .lrc
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
    });
    let mut tagged_file =
        lofty::read_from_path(path).map_err(|error| format!("读取音频文件失败: {error}"))?;

    // lofty 0.22 写 ID3v1 时可能在多字节字符中间按字节截断，触发 panic。
    // 主标签保存完整字段，移除 128 字节的旧标签不会丢掉本次写入的信息。
    if tagged_file.remove(TagType::Id3v1).is_some() {
        log::info!("已移除 ID3v1 标签，避免多字节字符截断引发崩溃");
    }

    let tag_type = tagged_file.primary_tag_type();
    if tagged_file.primary_tag().is_none() {
        tagged_file.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged_file
        .primary_tag_mut()
        .ok_or_else(|| "无法获取音频标签".to_string())?;

    // 标签字段来自任务上下文。先清掉旧值，避免空字段保留下载源中的错误信息。
    for (key, value) in [
        (ItemKey::TrackTitle, &context.song_info.title),
        (ItemKey::TrackArtist, &context.song_info.artist),
        (ItemKey::AlbumTitle, &context.song_info.album),
    ] {
        tag.remove_key(&key);
        if !value.is_empty() {
            tag.insert_text(key, value.clone());
        }
    }
    if let Some(lyric) = lyric_text {
        tag.remove_key(&ItemKey::Lyrics);
        tag.insert_text(ItemKey::Lyrics, lyric.to_string());
    }
    if let Some(bytes) = &assets.cover_bytes {
        let mut picture = Picture::from_reader(&mut bytes.as_slice())
            .map_err(|error| format!("读取封面图片失败: {error}"))?;
        picture.set_pic_type(PictureType::CoverFront);
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(picture);
    }

    let temporary = path.with_file_name(format!(
        ".hotdownloader-tags-{:x}.tmp",
        rand::random::<u64>()
    ));
    let result = (|| {
        std::fs::copy(path, &temporary)
            .map_err(|error| format!("创建标签临时文件失败: {error}"))?;
        tagged_file
            .save_to_path(&temporary, WriteOptions::default())
            .map_err(|error| format!("保存 metadata 失败: {error}"))?;
        let saved = lofty::probe::Probe::open(&temporary)
            .map_err(|error| format!("校验 metadata 失败: {error}"))?
            .set_file_type(tagged_file.file_type())
            .read()
            .map_err(|error| format!("校验 metadata 失败: {error}"))?;
        let tag = saved
            .primary_tag()
            .ok_or("校验 metadata 失败：主标签缺失")?;
        for (key, value) in [
            (ItemKey::TrackTitle, &context.song_info.title),
            (ItemKey::TrackArtist, &context.song_info.artist),
            (ItemKey::AlbumTitle, &context.song_info.album),
        ] {
            if !value.is_empty() && tag.get_string(&key) != Some(value.as_str()) {
                return Err(format!("校验 metadata 失败：{key:?} 未正确保存"));
            }
        }
        if lyric_text.is_some_and(|text| tag.get_string(&ItemKey::Lyrics) != Some(text)) {
            return Err("校验 metadata 失败：歌词未正确保存".into());
        }
        if assets.cover_bytes.as_deref().is_some_and(|bytes| {
            !tag.pictures().iter().any(|picture| {
                picture.pic_type() == PictureType::CoverFront && picture.data() == bytes
            })
        }) {
            return Err("校验 metadata 失败：封面未正确保存".into());
        }
        std::fs::rename(&temporary, path).map_err(|error| format!("替换标签文件失败: {error}"))?;
        Ok::<_, String>(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result?;
    log::info!("metadata 已写入: {}", path.display());
    Ok(true)
}

/// 普通文件系统的完整收尾实现，供独立进程持有。
pub struct LocalDownloadPostprocessor;

impl DownloadPostprocessor for LocalDownloadPostprocessor {
    fn process<'a>(
        &'a self,
        request: PostprocessRequest<'a>,
        progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Result<Vec<String>, String>> {
        Box::pin(async move {
            let _ = progress_sink;
            let assets = prepare_assets(request.context, request.config).await?;
            write_local_assets(&request, &assets)
        })
    }
}

pub(crate) fn write_local_assets(
    request: &PostprocessRequest<'_>,
    assets: &PostprocessAssets,
) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    // 获取完整资源后再写入；任何启用的产物写入失败均返回给 worker。
    if request.config.write_metadata {
        write_audio_metadata(Path::new(request.audio_path), request.context, assets)?;
    }

    if request.config.download_cover {
        if let Some(bytes) = assets.cover_bytes.as_deref() {
            paths.push(write_local_cover(request.audio_path, bytes).ok_or("保存封面文件失败")?);
        } else {
            log::info!("无可用封面，跳过封面文件创建");
        }
    }

    if request.config.download_lrc {
        if let Some(lyric) = assets.lyric.as_ref() {
            if lyric
                .lrc
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty())
            {
                paths.push(write_local_lrc(request.audio_path, lyric).ok_or("保存歌词文件失败")?);
            }
        } else {
            log::info!("无普通歌词，跳过 LRC 文件创建");
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::{
        write_audio_metadata, write_local_cover, write_local_lrc, LocalDownloadPostprocessor,
    };
    use crate::download::config::DownloadConfig;
    use crate::download::context::{SongInfo, TaskContext};
    use crate::download::ports::{DownloadPostprocessor, DownloadProgressSink, PostprocessRequest};
    use crate::download::postprocess::PostprocessAssets;
    use crate::platforms::kuwo::lyric_decoder;
    use crate::platforms::lyric::LyricData;
    use crate::platforms::Platform;
    use lofty::file::TaggedFileExt;
    use lofty::tag::ItemKey;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct UnexpectedProgress;

    impl DownloadProgressSink for UnexpectedProgress {
        fn progress(&self, _: &str, _: u64, _: u64, _: u64) {
            panic!("unexpected progress");
        }
        fn file_complete(&self, _: &str) {
            panic!("unexpected file completion");
        }
        fn completed(&self, _: &str, _: &str, _: Option<String>) {
            panic!("unexpected completion");
        }
        fn error(&self, _: &str, message: &str) {
            panic!("unexpected error: {message}");
        }
        fn link_expired(&self, _: &str, _: u64) {
            panic!("unexpected expired link");
        }
        fn metadata_error(&self, _: &str, message: &str) {
            panic!("unexpected metadata error: {message}");
        }
    }

    #[tokio::test]
    async fn unavailable_and_instrumental_lyrics_preserve_cover_and_metadata_processing() {
        use crate::download::postprocess::prepare_assets_with_lyric;
        use crate::platforms::qqmusic::lyrics::parse_decrypted_lyric;
        use base64::Engine;
        use std::io::{Read, Write};
        use std::time::{Duration, Instant};

        let image = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a9X8AAAAASUVORK5CYII=").unwrap();
        let cases = [
            (
                r#"<QrcInfos><Lyric_1 LyricContent=""/></QrcInfos>"#,
                true,
                Some("[00:00.00]暂无歌词\n"),
            ),
            (
                "[00:00.00]纯音乐，请欣赏",
                true,
                Some("[00:00.00]纯音乐，请欣赏"),
            ),
            (
                r#"<QrcInfos><Lyric_1 LyricContent=""/></QrcInfos>"#,
                false,
                None,
            ),
        ];
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let cover_url = format!("http://{}/cover.png", listener.local_addr().unwrap());
        let response_image = image.clone();
        let requests = cases.len();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            for _ in 0..requests {
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("cover request failed: {error}"),
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
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response_image.len()
                )
                .unwrap();
                stream.write_all(&response_image).unwrap();
            }
        });
        let directory = std::env::temp_dir().join(format!(
            "hotdownloader-no-lyrics-{:x}",
            rand::random::<u64>()
        ));
        std::fs::create_dir(&directory).unwrap();
        for (index, (plaintext, download_lrc, expected)) in cases.into_iter().enumerate() {
            let audio = directory.join(format!("no-lyrics-{index}.mp3"));
            std::fs::write(
                &audio,
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../hotdownloader-server/testdata/audio/silence.mp3"
                )),
            )
            .unwrap();
            let context = TaskContext {
                task_id: format!("no-lyrics-{index}"),
                platform: Platform::QqMusic,
                song_id: 1,
                song_mid: "fixture".into(),
                url: String::new(),
                save_path: audio.to_string_lossy().into(),
                quality: "320kmp3".into(),
                quality_filename: "fixture.mp3".into(),
                key: String::new(),
                file_size: 0,
                downloaded_offset: 0,
                song_info: SongInfo {
                    title: "无歌词测试".into(),
                    artist: "测试歌手".into(),
                    album: "测试专辑".into(),
                    quality: "320kmp3".into(),
                    cover_url: cover_url.clone(),
                },
                final_path: Arc::new(Mutex::new(None)),
            };
            let config = DownloadConfig::from_settings(
                &serde_json::json!({"writeMetadata":true,"downloadCover":true,"downloadLrc":download_lrc}),
                directory.to_str().unwrap(),
            );
            let calls = std::sync::atomic::AtomicUsize::new(0);
            let assets = prepare_assets_with_lyric(&context, &config, || async {
                calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                parse_decrypted_lyric(plaintext)
            })
            .await
            .unwrap();
            assert_eq!(
                calls.load(std::sync::atomic::Ordering::Relaxed),
                1,
                "正常无歌词不重试"
            );
            let paths = super::write_local_assets(
                &PostprocessRequest {
                    context: &context,
                    audio_path: audio.to_str().unwrap(),
                    is_saf: false,
                    saf_file_uri: None,
                    saf_folder_uri: None,
                    config: &config,
                },
                &assets,
            )
            .unwrap();
            assert_eq!(paths.len(), if download_lrc { 2 } else { 1 });
            assert_eq!(std::fs::read(audio.with_extension("png")).unwrap(), image);
            let saved = lofty::read_from_path(&audio).unwrap();
            let tag = saved.primary_tag().unwrap();
            assert_eq!(tag.get_string(&ItemKey::TrackTitle), Some("无歌词测试"));
            assert_eq!(tag.pictures()[0].data(), image);
            if let Some(expected) = expected {
                assert_eq!(
                    std::fs::read_to_string(audio.with_extension("lrc")).unwrap(),
                    expected
                );
                assert_eq!(tag.get_string(&ItemKey::Lyrics), Some(expected));
            } else {
                assert!(!audio.with_extension("lrc").exists());
            }
            for path in paths {
                std::fs::remove_file(path).unwrap();
            }
            std::fs::remove_file(audio).unwrap();
        }
        server.join().unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[tokio::test]
    async fn cover_download_respects_toggle_and_preserves_audio_without_metadata() {
        use base64::Engine;
        use std::io::{Read, Write};
        use std::time::Duration;

        let image = base64::engine::general_purpose::STANDARD.decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a9X8AAAAASUVORK5CYII="
        ).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        // URL 故意使用 jpg，确认最终扩展名依据响应图片内容。
        let cover_url = format!("http://{}/cover.jpg", listener.local_addr().unwrap());
        let response_image = image.clone();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5));
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
            assert!(String::from_utf8_lossy(&request).starts_with("GET /cover.jpg "));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_image.len()
            )
            .unwrap();
            stream.write_all(&response_image).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("hotdownloader-cover-{:x}", rand::random::<u64>()));
        std::fs::create_dir_all(&directory).unwrap();
        let audio = directory.join("歌曲.live - 歌手 (2).flac");
        std::fs::write(&audio, b"audio sentinel").unwrap();
        let context = TaskContext {
            task_id: "cover-download".into(),
            platform: Platform::QqMusic,
            song_mid: "synthetic".into(),
            song_id: 1,
            url: String::new(),
            save_path: audio.to_string_lossy().into(),
            quality: "flac".into(),
            key: String::new(),
            file_size: 14,
            downloaded_offset: 0,
            quality_filename: String::new(),
            final_path: Arc::new(Mutex::new(None)),
            song_info: SongInfo {
                title: "歌曲".into(),
                artist: "歌手".into(),
                album: String::new(),
                quality: "flac".into(),
                cover_url,
            },
        };
        for enabled in [false, true] {
            let config = DownloadConfig::from_settings(
                &serde_json::json!({"downloadCover": enabled}),
                directory.to_str().unwrap(),
            );
            let paths = LocalDownloadPostprocessor
                .process(
                    PostprocessRequest {
                        context: &context,
                        audio_path: audio.to_str().unwrap(),
                        is_saf: false,
                        saf_file_uri: None,
                        saf_folder_uri: None,
                        config: &config,
                    },
                    &UnexpectedProgress,
                )
                .await
                .unwrap();
            if enabled {
                assert_eq!(
                    paths,
                    [directory.join("歌曲.live - 歌手 (2).png").to_string_lossy()]
                );
                assert_eq!(std::fs::read(&paths[0]).unwrap(), image);
                // 覆盖同名文件时截断旧内容，不留下尾部字节。
                std::fs::write(&paths[0], vec![0; image.len() + 100]).unwrap();
                assert_eq!(
                    write_local_cover(audio.to_str().unwrap(), &image),
                    Some(paths[0].clone())
                );
                assert_eq!(std::fs::read(&paths[0]).unwrap(), image);
                std::fs::remove_file(&paths[0]).unwrap();
            } else {
                assert!(paths.is_empty());
                assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
            }
            assert_eq!(std::fs::read(&audio).unwrap(), b"audio sentinel");
        }
        assert!(write_local_cover(audio.to_str().unwrap(), b"<html>error</html>").is_none());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        server.join().unwrap();
        std::fs::remove_file(audio).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn bilingual_word_times_survive_mp3_and_flac_metadata_and_plain_sidecar() {
        use base64::Engine;
        // 使用现有的合成静音音频，验证实际落盘后的标签，避免只验证转换函数。
        let files: [(&str, &[u8]); 2] = [
            (
                "mp3",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../hotdownloader-server/testdata/audio/silence.mp3"
                )),
            ),
            (
                "flac",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../hotdownloader-server/testdata/audio/silence.flac"
                )),
            ),
        ];
        let directory = std::env::temp_dir().join(format!(
            "hotdownloader-lyrics-tags-{:x}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let raw = "[00:00.000]<500,-500>First <1000,0>line\n[00:01.000]<0,0>第一行\n[00:01.000]<500,-500>Second <1000,0>line\n[00:02.000]<0,0>第二行\n[00:02.000]";
        let (tags, lines, enhanced) = lyric_decoder::parse_lyric(raw);
        let lrc = lyric_decoder::build_lrc_text(&tags, &lines);
        let elrc = lyric_decoder::build_enhanced_lrc_text(&tags, &enhanced);
        let assets = PostprocessAssets {
            lyric: Some(LyricData {
                lrc: Some(lrc.clone()),
                elrc: Some(elrc.clone()),
                ..LyricData::default()
            }),
            cover_bytes: Some(base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a9X8AAAAASUVORK5CYII=").unwrap()),
        };
        for (extension, bytes) in files {
            let audio = directory.join(format!("bilingual.{extension}"));
            std::fs::write(&audio, bytes).unwrap();
            let context = TaskContext {
                task_id: "lyric-round-trip".into(),
                platform: Platform::Kuwo,
                song_mid: "synthetic".into(),
                song_id: 1,
                url: String::new(),
                save_path: audio.to_string_lossy().into(),
                quality: "128kmp3".into(),
                key: String::new(),
                file_size: bytes.len() as u64,
                downloaded_offset: 0,
                quality_filename: String::new(),
                final_path: Arc::new(Mutex::new(None)),
                song_info: SongInfo {
                    title: "测试歌曲".into(),
                    artist: "测试歌手".into(),
                    album: "测试专辑".into(),
                    quality: "128kmp3".into(),
                    cover_url: String::new(),
                },
            };
            // 即使歌曲没有歌词和封面，标题、歌手和专辑仍必须写入。
            write_audio_metadata(
                &audio,
                &context,
                &PostprocessAssets {
                    lyric: None,
                    cover_bytes: None,
                },
            )
            .unwrap();
            assert_eq!(
                lofty::read_from_path(&audio)
                    .unwrap()
                    .primary_tag()
                    .unwrap()
                    .get_string(&ItemKey::TrackTitle),
                Some("测试歌曲")
            );
            let before = std::fs::read(&audio).unwrap();
            assert!(write_audio_metadata(
                &audio,
                &context,
                &PostprocessAssets {
                    lyric: None,
                    cover_bytes: Some(b"not an image".to_vec())
                }
            )
            .is_err());
            assert_eq!(std::fs::read(&audio).unwrap(), before);
            // Docker、本地 Tauri 和 Android SAF 临时文件都调用同一个标签写入函数。
            assert!(write_audio_metadata(&audio, &context, &assets).unwrap());
            assert!(write_audio_metadata(&audio, &context, &assets).unwrap());
            let saved = lofty::read_from_path(&audio).unwrap();
            let tag = saved.primary_tag().unwrap();
            assert_eq!(tag.get_string(&ItemKey::Lyrics), Some(elrc.as_str()));
            assert_eq!(tag.get_strings(&ItemKey::Lyrics).count(), 1);
            assert_eq!(tag.pictures().len(), 1);
            assert_eq!(
                tag.pictures()[0].data(),
                assets.cover_bytes.as_deref().unwrap()
            );
            let path =
                write_local_lrc(audio.to_str().unwrap(), assets.lyric.as_ref().unwrap()).unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), lrc);
            std::fs::remove_file(path).unwrap();
            std::fs::remove_file(audio).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn ordinary_lrc_uses_audio_stem_and_preserves_content() {
        let directory =
            std::env::temp_dir().join(format!("hotdownloader-lrc-{:x}", rand::random::<u64>()));
        std::fs::create_dir_all(&directory).unwrap();
        let audio = directory.join("歌名.flac");
        let lyric = LyricData {
            lrc: Some("[00:01.00]歌词\n".into()),
            ..LyricData::default()
        };

        let written = write_local_lrc(audio.to_str().unwrap(), &lyric).unwrap();
        assert_eq!(std::path::Path::new(&written), directory.join("歌名.lrc"));
        assert_eq!(
            std::fs::read_to_string(&written).unwrap(),
            "[00:01.00]歌词\n"
        );

        std::fs::remove_file(written).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}

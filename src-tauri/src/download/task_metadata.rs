use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::tag::{ItemKey, Tag, TagType};
use tauri::AppHandle;
use tauri_plugin_android_fs::{AndroidFsExt, FileAccessMode, FsUri};

use super::progress;
use crate::platforms::lyric::LyricData;
use crate::utils::http::CLIENT; // 全局 HTTP 客户端，用于下载封面

/// 将歌词与封面写入音频文件 metadata
/// 普通模式直接操作文件路径；SAF 模式通过临时文件回写实现跨平台支持
/// 错误时通过 progress::emit_metadata_error 发送提示事件，不阻断下载完成事件
pub(crate) async fn write_metadata(
    app_handle: &AppHandle,
    task_id: &str,
    file_path: &str,
    is_saf: bool,
    saf_file_uri: Option<String>,
    song_title: &str,
    song_artist: &str,
    song_album: &str,
    cover_url: &str,
    lyric: Option<LyricData>,
) {
    // 1. 从已获取的歌词响应中提取歌词内容：优先逐字歌词（elrc），其次普通歌词（lrc）
    let lyric_text = lyric.and_then(|resp| {
        // 优先级：逐字歌词（elrc） → 普通歌词（lrc）
        if let Some(elrc) = resp.elrc.filter(|s| !s.trim().is_empty()) {
            Some(elrc)
        } else {
            resp.lrc.filter(|s| !s.trim().is_empty())
        }
    });

    // 2. 下载封面图片字节
    let cover_bytes = if !cover_url.is_empty() {
        match CLIENT.get(cover_url).send().await {
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
                progress::emit_metadata_error(app_handle, task_id, "SAF 文件 URI 缺失");
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
                progress::emit_metadata_error(
                    app_handle,
                    task_id,
                    &format!("打开 SAF 文件读取失败: {}", e),
                );
                return;
            }
        };
        let mut buf = Vec::new();
        if let Err(e) = src.read_to_end(&mut buf) {
            log::warn!("读取 SAF 文件失败: {}", e);
            progress::emit_metadata_error(
                app_handle,
                task_id,
                &format!("读取 SAF 文件失败: {}", e),
            );
            return;
        }
        // 从原始文件名提取扩展名，保证临时文件能被 lofty 正确识别格式
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
            progress::emit_metadata_error(app_handle, task_id, &format!("写入临时文件失败: {}", e));
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
            progress::emit_metadata_error(app_handle, task_id, &format!("读取音频文件失败: {}", e));
            return;
        }
    };

    // 尽量保留 ID3v1 标签，仅当待写入歌词含有多字节字符（如中文）且文件存在 ID3v1 时，主动移除 ID3v1，避免 lofty 保存时因编码转换导致 panic。
    let needs_remove_id3v1 = lyric_text
        .as_ref()
        .is_some_and(|text| text.chars().any(|c| c as u32 > 0xFF));
    if needs_remove_id3v1 && tagged_file.remove(TagType::Id3v1).is_some() {
        log::info!("歌词包含非 Latin-1 字符，已移除 ID3v1 标签");
    }

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
            progress::emit_metadata_error(app_handle, task_id, "无法获取音频标签");
            return;
        }
    };

    // 写入歌曲标题、艺术家、专辑，覆盖原始文件中的信息。
    // 这些字段来自下载任务的 SongInfo，确保音频文件显示正确的元数据。
    tag.remove_key(&ItemKey::TrackTitle);
    if !song_title.is_empty() {
        tag.insert_text(ItemKey::TrackTitle, song_title.to_string());
    }
    tag.remove_key(&ItemKey::TrackArtist);
    if !song_artist.is_empty() {
        tag.insert_text(ItemKey::TrackArtist, song_artist.to_string());
    }
    tag.remove_key(&ItemKey::AlbumTitle);
    if !song_album.is_empty() {
        tag.insert_text(ItemKey::AlbumTitle, song_album.to_string());
    }

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
        if is_saf {
            let _ = std::fs::remove_file(&temp_path);
        }
        progress::emit_metadata_error(app_handle, task_id, &format!("保存 metadata 失败: {}", e));
        return;
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
                            progress::emit_metadata_error(
                                app_handle,
                                task_id,
                                &format!("读取临时文件失败: {}", e),
                            );
                            return;
                        }
                    };
                    // 清空原文件并从头写入，避免旧数据残留
                    if let Err(e) = dst.set_len(0) {
                        log::warn!("清空 SAF 文件失败: {}", e);
                        let _ = std::fs::remove_file(&temp_path);
                        progress::emit_metadata_error(
                            app_handle,
                            task_id,
                            &format!("清空 SAF 文件失败: {}", e),
                        );
                        return;
                    }
                    if let Err(e) = dst.seek(std::io::SeekFrom::Start(0)) {
                        log::warn!("SAF 文件 seek 失败: {}", e);
                        let _ = std::fs::remove_file(&temp_path);
                        progress::emit_metadata_error(
                            app_handle,
                            task_id,
                            &format!("SAF 文件 seek 失败: {}", e),
                        );
                        return;
                    }
                    if let Err(e) = dst.write_all(&data) {
                        log::warn!("写入 SAF 文件失败: {}", e);
                        let _ = std::fs::remove_file(&temp_path);
                        progress::emit_metadata_error(
                            app_handle,
                            task_id,
                            &format!("写入 SAF 文件失败: {}", e),
                        );
                        return;
                    }
                }
                Err(e) => {
                    log::warn!("打开 SAF 文件写入失败: {}", e);
                    let _ = std::fs::remove_file(&temp_path);
                    progress::emit_metadata_error(
                        app_handle,
                        task_id,
                        &format!("打开 SAF 文件写入失败: {}", e),
                    );
                    return;
                }
            }
        }
        let _ = std::fs::remove_file(&temp_path);
    }
}

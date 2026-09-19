use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::tag::{ItemKey, Tag, TagType};
use tauri::AppHandle;
use tauri::Manager;
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

        // 使用应用专属的缓存目录，而不是系统临时目录
        let temp_dir = app_handle
            .path()
            .app_cache_dir()
            .expect("Failed to get app cache dir");

        // 确保目录存在
        std::fs::create_dir_all(&temp_dir).ok();

        // 临时文件路径
        let temp = temp_dir.join(format!(
            "{}.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            ext
        ));

        // 写入临时文件
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

    // 始终移除 ID3v1 标签（重要：这是应用“崩溃”的根因之一）。
    //
    // lofty 0.22 在写 ID3v1 时，用 `val.split_at(30)` 按【字节】截断标题/歌手/专辑
    // （见 lofty/src/id3/v1/write.rs 的 resize_string；ID3v1 字段固定 30 字节）。
    // 只要字符串超过 30 字节且第 30 个字节正好落在多字节字符（中文/日文/带变音符号的
    // 拉丁字母等）中间，split_at 就会 panic：
    //   "end byte index 30 is not a char boundary; it is inside 'è' (bytes 29..31 of string)"
    // release 构建带 panic = "abort"，该 panic 会让整个应用瞬间退出（表现为“崩溃”）。
    //
    // 旧实现只在“歌词含多字节字符”时才移除 ID3v1，标题/歌手/专辑含中文时依旧会触发。
    // ID3v1 是 128 字节的遗留标签（仅支持 Latin-1、字段上限 30 字节），而标题/歌手/专辑/
    // 歌词都会写入主标签（MP3 为 ID3v2），因此直接移除它不会丢失信息。
    if tagged_file.remove(TagType::Id3v1).is_some() {
        log::info!("已移除 ID3v1 标签（避免其 30 字节字段按字节截断多字节字符时崩溃）");
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

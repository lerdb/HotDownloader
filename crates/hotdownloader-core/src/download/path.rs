use std::path::Path;

use super::filename;
use crate::download::context::SongInfo;

/// 根据任务设置计算最终文件位置，返回 SAF 标志、路径或文件名、SAF 文件夹 URI。
/// SAF 只在 Android 构建中生效；普通服务端始终使用本地文件系统路径。
pub fn resolve_download_path(
    dir_setting: &str,
    template_setting: &str,
    saf_uri_setting: Option<&str>,
    song_info: &SongInfo,
    quality_filename: &str,
) -> Result<(bool, String, Option<String>), String> {
    let stem = filename::build_filename(template_setting, song_info);
    let raw_ext = Path::new(quality_filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("flac");
    let extension = map_decrypted_extension(raw_ext);

    if dir_setting == "saf://" && cfg!(target_os = "android") && saf_uri_setting.is_some() {
        // SAF 适配器需要相对文件名及文件夹 URI，不能提前拼成普通文件系统路径。
        Ok((
            true,
            bounded_path(Path::new(""), &stem, extension, "", true)?,
            saf_uri_setting.map(str::to_string),
        ))
    } else {
        Ok((
            false,
            bounded_path(Path::new(dir_setting), &stem, extension, "", false)?,
            None,
        ))
    }
}

/// Windows 全路径预算包含当前目录和分隔符；其他平台仍统一限制单个文件名。
pub fn filename_unit_budget(parent: &Path, is_saf: bool) -> Result<usize, String> {
    if is_saf || !cfg!(windows) {
        return Ok(filename::MAX_COMPONENT_LENGTH);
    }
    let absolute = if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("无法解析下载目录: {error}"))?
            .join(parent)
    };
    let text = absolute.to_string_lossy();
    let text = text
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .unwrap_or_else(|| text.strip_prefix(r"\\?\").unwrap_or(&text).to_string());
    let separator = usize::from(!text.ends_with(['\\', '/']));
    Ok(259usize
        .saturating_sub(text.encode_utf16().count() + separator)
        .min(filename::MAX_COMPONENT_LENGTH))
}

pub fn bounded_path(
    parent: &Path,
    stem: &str,
    extension: &str,
    suffix: &str,
    is_saf: bool,
) -> Result<String, String> {
    let name = filename::bounded_filename(
        stem,
        extension,
        suffix,
        filename_unit_budget(parent, is_saf)?,
    )?;
    Ok(if is_saf {
        name
    } else {
        parent.join(name).to_string_lossy().into_owned()
    })
}

pub fn numbered_path(original: &str, number: usize, is_saf: bool) -> Result<String, String> {
    let path = Path::new(original);
    bounded_path(
        path.parent().unwrap_or_else(|| Path::new(".")),
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("无效文件名")?,
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or(""),
        &format!(" ({number})"),
        is_saf,
    )
}

pub fn truncation_warning(path: &str) -> Option<String> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    filename::truncated_digest(stem)?;
    Some(format!(
        "文件名过长已做截断处理：{}",
        Path::new(path).file_name()?.to_string_lossy()
    ))
}

/// 加密扩展名在落盘时对应实际可播放的音频格式。
pub fn map_decrypted_extension(extension: &str) -> &str {
    match extension {
        "mgg" => "ogg",
        "mflac" => "flac",
        _ => extension,
    }
}

#[cfg(test)]
mod tests {
    use super::{map_decrypted_extension, resolve_download_path};
    use crate::download::context::SongInfo;

    #[test]
    fn encrypted_extension_uses_playable_output_format() {
        assert_eq!(map_decrypted_extension("mgg"), "ogg");
        assert_eq!(map_decrypted_extension("mflac"), "flac");
    }

    #[test]
    fn path_uses_sanitized_template_and_decrypted_extension() {
        let info = SongInfo {
            title: "a/b".into(),
            artist: "Singer".into(),
            album: String::new(),
            quality: String::new(),
            cover_url: String::new(),
        };
        let (is_saf, path, uri) = resolve_download_path(
            "downloads",
            "{song} - {artist}",
            None,
            &info,
            "encrypted.mflac",
        )
        .unwrap();

        assert!(!is_saf);
        assert_eq!(
            path,
            std::path::Path::new("downloads")
                .join("a_b - Singer.flac")
                .to_string_lossy()
        );
        assert!(uri.is_none());
    }

    #[test]
    fn long_directory_budget_covers_numbering_and_sidecars() {
        let parent = std::env::temp_dir()
            .join("d".repeat(65))
            .join("e".repeat(65));
        let path = super::bounded_path(&parent, &"长歌名🎵".repeat(100), "mp3", "", false).unwrap();
        let numbered = super::numbered_path(&path, 9999, false).unwrap();
        for path in [&path, &numbered] {
            let name = std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            assert!(
                name.encode_utf16().count() <= super::filename_unit_budget(&parent, false).unwrap()
            );
            assert!(name.len() <= 255);
            assert!(super::truncation_warning(path).is_some());
            assert!(
                std::path::Path::new(path)
                    .with_extension("jpeg")
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .len()
                    <= 255
            );
        }
        assert_ne!(path, numbered);
    }

    #[cfg(windows)]
    #[test]
    fn an_unfixable_directory_is_reported_before_file_creation() {
        assert!(super::bounded_path(
            std::path::Path::new(&format!("C:\\{}", "d".repeat(255))),
            "song",
            "flac",
            "",
            false
        )
        .unwrap_err()
        .contains("缩短下载目录"));
    }
}

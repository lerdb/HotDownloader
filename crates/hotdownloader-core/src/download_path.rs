use std::path::Path;

use crate::filename;
use crate::task_context::SongInfo;

/// 根据任务设置计算最终文件位置，返回 SAF 标志、路径或文件名、SAF 文件夹 URI。
/// SAF 只在 Android 构建中生效；普通服务端始终使用本地文件系统路径。
pub fn resolve_download_path(
    dir_setting: &str,
    template_setting: &str,
    saf_uri_setting: Option<&str>,
    song_info: &SongInfo,
    quality_filename: &str,
) -> (bool, String, Option<String>) {
    let stem = filename::build_filename(template_setting, song_info);
    let raw_ext = Path::new(quality_filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("flac");
    let file_name = format!("{}.{}", stem, map_decrypted_extension(raw_ext));

    if dir_setting == "saf://" && cfg!(target_os = "android") && saf_uri_setting.is_some() {
        // SAF 适配器需要相对文件名及文件夹 URI，不能提前拼成普通文件系统路径。
        (true, file_name, saf_uri_setting.map(str::to_string))
    } else {
        let full_path = Path::new(dir_setting).join(file_name);
        (false, full_path.to_string_lossy().to_string(), None)
    }
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
    use crate::task_context::SongInfo;

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
        );

        assert!(!is_saf);
        assert_eq!(
            path,
            std::path::Path::new("downloads")
                .join("a_b - Singer.flac")
                .to_string_lossy()
        );
        assert!(uri.is_none());
    }
}

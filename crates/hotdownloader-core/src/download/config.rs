use std::path::Path;

/// 下载封面独立于列表缩略图；原图不可用时按平台支持的尺寸回退。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoverQuality {
    Source,
    High,
    #[default]
    Original,
}

impl CoverQuality {
    fn from_settings(settings: &serde_json::Value) -> Self {
        match settings
            .get("coverQuality")
            .and_then(|value| value.as_str())
        {
            Some("source") => Self::Source,
            Some("500") => Self::High,
            _ => Self::Original,
        }
    }
}

/// 下载任务启动时使用的设置快照。任务运行期间保持同一份设置，避免中途改动目录或文件名。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadConfig {
    pub download_dir: String,
    pub naming_template: String,
    pub saf_folder_uri: Option<String>,
    pub write_metadata: bool,
    pub cover_quality: CoverQuality,
    pub download_lrc: bool,
    pub download_cover: bool,
}

impl DownloadConfig {
    /// 只解析业务字段；设置的读取方式与系统默认目录由运行时适配器提供。
    pub fn from_settings(settings: &serde_json::Value, default_dir: &str) -> Self {
        let requested_dir = settings
            .get("downloadDir")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or(default_dir);

        // Android 应用私有目录不可作为用户下载目录；相对路径也不能稳定地跨重启使用。
        let download_dir =
            if requested_dir.contains("/data/user/0/") || requested_dir.contains("/data/data/") {
                log::warn!("检测到应用私有目录路径，已回退为默认下载目录: {requested_dir}");
                default_dir.to_string()
            } else if Path::new(requested_dir).is_absolute() || requested_dir == "saf://" {
                requested_dir.to_string()
            } else {
                log::warn!("下载目录不是绝对路径，已回退为默认下载目录: {default_dir}");
                default_dir.to_string()
            };

        Self {
            download_dir,
            naming_template: settings
                .get("namingTemplate")
                .and_then(|value| value.as_str())
                .unwrap_or("{song} - {artist}")
                .to_string(),
            saf_folder_uri: settings
                .get("safFolderUri")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            write_metadata: settings
                .get("writeMetadata")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            download_lrc: settings
                .get("downloadLrc")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            download_cover: settings
                .get("downloadCover")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            cover_quality: CoverQuality::from_settings(settings),
        }
    }
}

/// 运行时负责读取设置，并提供适合当前平台的默认下载目录。
pub trait DownloadConfigProvider: Send + Sync {
    fn current(&self) -> DownloadConfig;
}

#[cfg(test)]
mod tests {
    use super::{CoverQuality, DownloadConfig};

    #[test]
    fn standalone_cover_defaults_off_and_is_independent_of_metadata() {
        let config = DownloadConfig::from_settings(&serde_json::json!({}), "C:/Music");
        assert!(!config.download_cover);
        let config = DownloadConfig::from_settings(
            &serde_json::json!({"downloadCover": true, "writeMetadata": false}),
            "C:/Music",
        );
        assert!(config.download_cover);
        assert!(!config.write_metadata);
    }

    #[test]
    fn cover_quality_defaults_to_original_and_reads_saved_choices() {
        for value in [
            serde_json::Value::Null,
            serde_json::json!("unknown"),
            serde_json::json!(500),
        ] {
            let config = DownloadConfig::from_settings(
                &serde_json::json!({"coverQuality": value}),
                "/tmp/music",
            );
            assert_eq!(config.cover_quality, CoverQuality::Original);
        }
        assert_eq!(
            DownloadConfig::from_settings(&serde_json::json!({}), "/tmp/music").cover_quality,
            CoverQuality::Original
        );
        for (value, expected) in [
            ("source", CoverQuality::Source),
            ("500", CoverQuality::High),
            ("original", CoverQuality::Original),
        ] {
            let config = DownloadConfig::from_settings(
                &serde_json::json!({"coverQuality": value}),
                "/tmp/music",
            );
            assert_eq!(config.cover_quality, expected);
        }
    }

    #[test]
    fn invalid_directory_falls_back_without_losing_other_settings() {
        let settings = serde_json::json!({
            "downloadDir": "relative/music",
            "namingTemplate": "{artist} - {song}",
            "writeMetadata": true,
            "downloadLrc": true,
        });
        let config = DownloadConfig::from_settings(&settings, "C:/Music");
        assert_eq!(config.download_dir, "C:/Music");
        assert_eq!(config.naming_template, "{artist} - {song}");
        assert!(config.write_metadata);
        assert!(config.download_lrc);
    }

    #[test]
    fn saf_directory_and_uri_are_preserved() {
        let settings = serde_json::json!({
            "downloadDir": "saf://",
            "safFolderUri": "content://folder",
        });
        let config = DownloadConfig::from_settings(&settings, "/tmp/music");
        assert_eq!(config.download_dir, "saf://");
        assert_eq!(config.saf_folder_uri.as_deref(), Some("content://folder"));
    }
}

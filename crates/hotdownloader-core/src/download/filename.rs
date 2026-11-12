use crate::download::context::SongInfo;
use md5::{Digest, Md5};

pub const MAX_COMPONENT_LENGTH: usize = 255;

pub fn name_digest(stem: &str) -> String {
    format!("{:x}", Md5::digest(stem.as_bytes()))[..16].to_string()
}

/// 识别由本模块生成的截断标记，供重命名、提示和标签优先的库扫描共用。
pub fn truncated_digest(stem: &str) -> Option<&str> {
    let (_, digest) = stem.strip_suffix("...")?.rsplit_once('~')?;
    (digest.len() == 16 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(digest)
}

/// 同时遵守 UTF-8 字节和 UTF-16 单元预算，预留歌词、封面的扩展名长度。
pub fn bounded_filename(
    stem: &str,
    extension: &str,
    suffix: &str,
    max_units: usize,
) -> Result<String, String> {
    let extension = sanitize_name(extension);
    let extension = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    let stem = if extension.is_empty() {
        stem.trim_end_matches([' ', '.'])
    } else {
        stem
    };
    if stem.is_empty() {
        return Err("无效文件名".into());
    }
    if extension.len() >= MAX_COMPONENT_LENGTH {
        return Err("文件扩展名过长，无法生成下载文件名".into());
    }
    let bytes = MAX_COMPONENT_LENGTH.saturating_sub(extension.len().max(5));
    let units = max_units
        .min(MAX_COMPONENT_LENGTH)
        .saturating_sub(extension.encode_utf16().count().max(5));
    let fits = |value: &str| value.len() <= bytes && value.encode_utf16().count() <= units;
    let candidate = format!("{stem}{suffix}");
    if fits(&candidate) {
        return Ok(format!("{candidate}{extension}"));
    }
    let digest = truncated_digest(stem)
        .map(str::to_owned)
        .unwrap_or_else(|| name_digest(stem));
    let base = if truncated_digest(stem).is_some() {
        &stem[..stem.len() - 20]
    } else {
        stem
    };
    let tail = format!("{suffix}~{digest}...");
    let mut shortened = String::new();
    for character in base.chars() {
        if shortened.len() + character.len_utf8() + tail.len() > bytes
            || shortened.encode_utf16().count()
                + character.len_utf16()
                + tail.encode_utf16().count()
                > units
        {
            break;
        }
        shortened.push(character);
    }
    if !fits(&format!("{shortened}{tail}")) {
        return Err("下载目录路径过长，无法生成安全的文件名；请缩短下载目录路径".into());
    }
    Ok(format!("{shortened}{tail}{extension}"))
}

/// 过滤各平台常见的非法文件名字符，保持已有文件命名规则。
pub fn sanitize_name(raw: &str) -> String {
    let mut name: String = raw
        .chars()
        .map(|character| {
            if character <= '\u{1f}'
                || ['\\', '/', ':', '*', '?', '"', '<', '>', '|'].contains(&character)
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let base = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_uppercase();
    let device = ["CON", "PRN", "AUX", "NUL"].contains(&base.as_str())
        || ["COM", "LPT"].iter().any(|prefix| {
            base.strip_prefix(prefix).is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    if device {
        name.insert(0, '_');
    }
    name
}

/// 将命名模板中的歌曲、歌手、专辑和音质变量替换为实际值。
pub fn apply_template(
    template: &str,
    artist: &str,
    title: &str,
    album: &str,
    quality: &str,
) -> String {
    template
        .replace("{song}", title)
        .replace("{artist}", artist)
        .replace("{album}", album)
        .replace("{quality}", quality)
}

/// 生成不含扩展名的文件名；模板无有效字符时使用原有的两级回退规则。
pub fn build_filename(template: &str, info: &SongInfo) -> String {
    let name = apply_template(
        template,
        &info.artist,
        &info.title,
        &info.album,
        &info.quality,
    );
    let sanitized = sanitize_name(&name);

    if sanitized.trim().is_empty() {
        // 用户模板可能只有非法字符，先退回默认模板；歌曲数据也为空时给出稳定名称。
        let fallback = apply_template(
            "{song} - {artist}",
            &info.artist,
            &info.title,
            &info.album,
            &info.quality,
        );
        let fallback_sanitized = sanitize_name(&fallback);
        if fallback_sanitized.trim().is_empty() {
            "未知歌曲".to_string()
        } else {
            fallback_sanitized
        }
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_bounds_preserve_unicode_extensions_and_distinguish_long_titles() {
        for stem in [
            "a".repeat(600),
            "长歌名与多位歌手".repeat(100),
            "音乐🎵😀".repeat(100),
        ] {
            let name = bounded_filename(&stem, "flac", "", 255).unwrap();
            assert!(name.len() <= 255 && name.encode_utf16().count() <= 255);
            assert!(name.ends_with("....flac"));
            let shortened = std::path::Path::new(&name)
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap();
            assert_eq!(
                truncated_digest(shortened),
                Some(name_digest(&stem).as_str())
            );
            assert_eq!(bounded_filename(&stem, "flac", "", 255).unwrap(), name);
            assert_ne!(
                bounded_filename(&(stem + "另一个版本"), "flac", "", 255).unwrap(),
                name
            );
        }
    }

    #[test]
    fn limits_include_numbering_and_the_longest_sidecar_extension() {
        let exact = "a".repeat(250);
        assert_eq!(
            bounded_filename(&exact, "flac", "", 255).unwrap(),
            format!("{exact}.flac")
        );
        for number in [1, 9, 99, 9999] {
            let name = bounded_filename(&exact, "mp3", &format!(" ({number})"), 255).unwrap();
            assert!(name.len() <= 254, "{name}");
            assert!(name.contains(&format!(" ({number})")));
            assert!(
                std::path::Path::new(&name)
                    .with_extension("jpeg")
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .len()
                    <= 255
            );
        }
        let limited = bounded_filename(&"😀".repeat(100), "flac", "", 40).unwrap();
        assert!(limited.encode_utf16().count() <= 40);
        let tight = bounded_filename(&"😀".repeat(100), "flac", "", 25).unwrap();
        assert_eq!(tight.encode_utf16().count(), 25);
        assert!(bounded_filename("song", "flac", "", 8)
            .unwrap_err()
            .contains("下载目录路径过长"));
    }

    #[test]
    fn windows_reserved_names_and_controls_are_safe_without_changing_normal_stems() {
        for reserved in ["CON", "nul.txt", "COM1", "LPT9", "COM¹"] {
            assert!(sanitize_name(reserved).starts_with('_'));
        }
        assert_eq!(sanitize_name("歌名 - 歌手..."), "歌名 - 歌手...");
        assert_eq!(sanitize_name("a\0b\nc/d"), "a_b_c_d");
        assert_eq!(
            bounded_filename("short", "mp3", "", 255).unwrap(),
            "short.mp3"
        );
        assert!(bounded_filename("", "mp3", "", 255).is_err());
    }
}

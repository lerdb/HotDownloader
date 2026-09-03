use super::super::download::task::SongInfo;
use tauri::AppHandle;

/// 默认的歌手分隔符。
pub const DEFAULT_ARTIST_SEPARATOR: &str = "、";

/// 过滤文件名中的非法字符
pub fn sanitize_name(raw: &str) -> String {
    raw.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_")
}

/// 从设置中读取歌手分隔符。
///
/// 解析失败、缺失或为空时，回退到 [`DEFAULT_ARTIST_SEPARATOR`]。
/// 该函数对解析错误保持静默，仅记录 warn 日志，避免在搜索/歌单等
/// 高频调用路径上引发噪音。
pub fn get_artist_separator(app_handle: &AppHandle) -> String {
    use crate::storage::store_wrapper;

    let raw = match store_wrapper::load_string(app_handle, "settings") {
        Ok(s) => s,
        Err(e) => {
            log::warn!("读取歌手分隔符设置失败: {}, 使用默认值", e);
            return DEFAULT_ARTIST_SEPARATOR.to_string();
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return DEFAULT_ARTIST_SEPARATOR.to_string(),
    };

    parsed
        .get("artistSeparator")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_ARTIST_SEPARATOR.to_string())
}

/// 应用命名模板，替换变量
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

/// 生成最终文件名（不含扩展名）
pub fn build_filename(template: &str, info: &SongInfo) -> String {
    let name = apply_template(
        template,
        &info.artist,
        &info.title,
        &info.album,
        &info.quality,
    );
    let sanitized = sanitize_name(&name);
    // 若过滤后为空，回退到默认模板
    if sanitized.trim().is_empty() {
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

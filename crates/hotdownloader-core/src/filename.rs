use crate::task_context::SongInfo;

/// 过滤各平台常见的非法文件名字符，保持已有文件命名规则。
pub fn sanitize_name(raw: &str) -> String {
    raw.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_")
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

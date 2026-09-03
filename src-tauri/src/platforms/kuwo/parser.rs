//! 酷我音乐歌曲解析与品质构建模块。
//!
//! 将酷我搜索接口返回的原始歌曲 JSON 转换为统一格式的歌曲信息对象，
//! 包括基础字段（ID、mid、标题、歌手、专辑、封面）和可用品质列表。
//!
//! 关键点：
//! - 酷我的 `MUSICRID` 形如 `MUSIC_6802907`，需去掉 `MUSIC_` 前缀转为数字 ID。
//! - 酷我没有字符串 mid，前端 `mid` 字段填数字 ID 的字符串形式。
//! - 品质从 `N_MINFO`（优先）或 `MINFO` 字段解析，按 `bitrate` → 品质标签映射。
//! - 封面 URL 需要通过独立接口（见 `cover.rs`）获取，本模块不做处理。

use regex::Regex;
use serde_json::{json, Value};

/// 通用歌曲解析函数。
///
/// 将酷我搜索接口返回的原始歌曲 JSON 对象转换为前端需要的统一格式。
///
/// # 参数
/// - `song`: 歌曲原始 JSON 对象（搜索接口中的一项）。
///
/// # 返回
/// - `Some(Value)`：成功解析的歌曲信息 JSON 对象，包含以下字段：
///   - `id`: 数字歌曲 ID（去掉 `MUSIC_` 前缀的数字部分）
///   - `mid`: 歌曲唯一标识（酷我用数字 ID 字符串代替）
///   - `title`: 歌曲标题
///   - `artist`: 歌手名（多个歌手以 `&` 连接）
///   - `album`: 专辑名
///   - `coverUrl`: 封面图片 URL（搜索阶段为空，由外部独立接口填充）
///   - `mediaMid`: 媒体文件标识（酷我用歌曲数字 ID）
///   - `qualities`: 可用品质列表，每项含 `quality`、`format`、`bitrate`、`size`、`filename`
/// - `None`：当歌曲缺少 `MUSICRID` 时返回 `None`，表示该歌曲无法解析或不可下载。
pub(crate) fn parse_song(song: &Value) -> Option<Value> {
    // 提取数字歌曲 ID（去掉 MUSIC_ 前缀）
    let music_rid = song["MUSICRID"].as_str().unwrap_or("");
    let id_str = music_rid.strip_prefix("MUSIC_").unwrap_or("");
    let song_id: u64 = match id_str.parse() {
        Ok(n) => n,
        Err(_) => return None,
    };
    if song_id == 0 {
        return None;
    }
    // mid 字段复用数字 ID 字符串
    let mid = song_id.to_string();

    // 标题：优先 SONGNAME，回退 NAME/FSONGNAME（去除 FSONGNAME 首尾的双引号）
    let title = song["SONGNAME"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            song["NAME"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            song["FSONGNAME"]
                .as_str()
                .map(|s| s.trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();

    // 歌手：优先 ARTIST（含 & 分隔的多歌手），回退 AARTIST（通常是英文名）
    let artist = song["ARTIST"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            song["AARTIST"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();

    // 专辑
    let album = song["ALBUM"].as_str().unwrap_or("").to_string();

    // 时长（秒），用于前端展示
    let duration = song["DURATION"].as_u64().unwrap_or(0);

    // 封面 URL 在搜索阶段为空，由搜索模块并发调用封面接口填充
    let cover_url = String::new();

    // 构建品质列表（优先解析 N_MINFO，回退 MINFO）
    let info_str = song["N_MINFO"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| song["MINFO"].as_str())
        .unwrap_or("");
    let qualities = build_qualities(info_str);

    Some(json!({
        "id": song_id,
        "mid": mid,
        "title": title,
        "artist": artist,
        "album": album,
        "duration": duration,
        "coverUrl": cover_url,
        "mediaMid": mid,    // 酷我用数字 ID 作为 mediaMid
        "qualities": qualities
    }))
}

/// 解析 `N_MINFO` 字符串，返回品质列表。
///
/// 每条品质记录格式：`level:xxx,bitrate:NNN,format:xxx,size:NNN`，用分号分隔。
///
/// # 参数
/// - `info_str`: 待解析的音质字符串。
///
/// # 返回
/// 品质项数组，每项为 JSON 对象 `{ quality, size, filename }`：
/// - `quality`: 品质标签（按 `bitrate` 映射，如 `"320kmp3"`/`"flac"`/`"臻品母带"`）。
/// - `size`: 字节数。
/// - `filename`: 编码为 `{bitrate}.{format}` 的本地保存文件名（如 `320.mp3`、`20900.mflac`）。
///   下载时 `kuwo::download` 从扩展名解析 format、从 stem 解析 bitrate，无需新增任何字段。
pub(crate) fn build_qualities(info_str: &str) -> Vec<Value> {
    if info_str.is_empty() {
        return Vec::new();
    }

    // 匹配单条品质记录
    let re = match Regex::new(r"level:(\w+),bitrate:(\d+),format:(\w+),size:([\w.]+)") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut list: Vec<Value> = Vec::new();

    // 使用 HashSet 去重（同 bitrate 取首次出现的条目，避免重复）
    let mut seen_bitrate: std::collections::HashSet<u32> = std::collections::HashSet::new();

    for cap in re.captures_iter(info_str) {
        let bitrate: u32 = match cap[2].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if !seen_bitrate.insert(bitrate) {
            continue;
        }

        let format = cap[3].to_string();
        let size_str = cap[4].to_string();
        // 解析 size：将 "21.93Mb" 转换为字节数
        let size = parse_size(&size_str);

        // 按用户提供的 bitrate → 品质标签映射
        let quality = match bitrate {
            20900 => "臻品母带",
            24000 => "臻品全景声 7.1.4",
            20501 => "臻品全景声 5.1",
            20201 => "臻品全景声",
            4000 => "hires",
            2000 => "flac",
            320 => "320kmp3",
            128 => "128kmp3",
            300 => "300kogg",
            192 => "192kogg",
            100 => "100kogg",
            48 => "48kaac",
            _ => continue, // 未知 bitrate 跳过
        };

        // filename 编码 bitrate 与 format，复用 QQ 的 filename 字段约定：
        // 例如 `320.mp3`、`2000.flac`、`20900.mflac`。
        // 后端下载时只需从 filename 解析出这两个参数，无需新增任务字段。
        let filename = format!("{}.{}", bitrate, format);

        list.push(json!({
            "quality": quality,
            "size": size,
            "filename": filename
        }));
    }

    list
}

/// 将 `21.93Mb`、`5.65Mb`、`134.68Mb` 等格式解析为字节数。
///
/// 单位支持 `b`（字节）、`kb`、`mb`、`gb`，不区分大小写。
///
/// # 参数
/// - `size_str`: 形如 `"21.93Mb"` 的字符串。
///
/// # 返回
/// 字节数；解析失败返回 0。
fn parse_size(size_str: &str) -> u64 {
    let re = match Regex::new(r"(?i)^([\d.]+)([kmg]?b)$") {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let cap = match re.captures(size_str) {
        Some(c) => c,
        None => return 0,
    };
    let num: f64 = cap[1].parse().unwrap_or(0.0);
    let unit = cap[2].to_lowercase();
    let multiplier: f64 = match unit.as_str() {
        "b" => 1.0,
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    (num * multiplier) as u64
}

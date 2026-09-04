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
//! - 酷我接口的 `ARTIST` 字段本身使用 `&` 作为歌手分隔符；解析后按用户设置的
//!   `artistSeparator` 重新拼接，保持与其他平台一致的展示形式。

use regex::Regex;
use serde_json::{json, Value};

/// 通用歌曲解析函数。
///
/// 将酷我搜索接口返回的原始歌曲 JSON 对象转换为前端需要的统一格式。
///
/// # 参数
/// - `song`: 歌曲原始 JSON 对象（搜索接口中的一项）。
/// - `artist_separator`: 多名歌手之间的连接字符串（来自 `artistSeparator` 设置）。
///   酷我原始 `ARTIST` 字段使用 `&` 分隔，本函数先按 `&` 拆分为数组，
///   再用 `artist_separator` 拼接为字符串，便于跨平台保持一致。
///
/// # 返回
/// - `Some(Value)`：成功解析的歌曲信息 JSON 对象，包含以下字段：
///   - `id`: 数字歌曲 ID（去掉 `MUSIC_` 前缀的数字部分）
///   - `mid`: 歌曲唯一标识（酷我用数字 ID 字符串代替）
///   - `title`: 歌曲标题
///   - `artist`: 歌手名（多个歌手以 `artist_separator` 连接）
///   - `album`: 专辑名
///   - `coverUrl`: 封面图片 URL（搜索阶段为空，由外部独立接口填充）
///   - `mediaMid`: 媒体文件标识（酷我用歌曲数字 ID）
///   - `qualities`: 可用品质列表，每项含 `quality`、`format`、`bitrate`、`size`、`filename`
/// - `None`：当歌曲缺少 `MUSICRID` 时返回 `None`，表示该歌曲无法解析或不可下载。
pub(crate) fn parse_song(song: &Value, artist_separator: &str) -> Option<Value> {
    // 提取数字歌曲 ID：优先使用 `id` 字段（歌单接口返回），若不存在则回退到 `MUSICRID`。
    let song_id: u64 = if let Some(id_val) = song.get("id") {
        // 优先从 `id` 字段解析：支持字符串和数字类型
        match id_val {
            Value::String(s) => s.parse().ok(),
            Value::Number(n) => n.as_u64(),
            _ => None,
        }
    } else {
        None
    }
    .or_else(|| {
        // 回退到 `MUSICRID` 字段
        let music_rid = song["MUSICRID"].as_str().unwrap_or("");
        if let Some(stripped) = music_rid.strip_prefix("MUSIC_") {
            // 带前缀：去掉前缀后解析
            stripped.parse().ok()
        } else {
            // 不带前缀：尝试直接解析为数字
            music_rid.parse().ok()
        }
    })
    .filter(|id| *id > 0)?; // 过滤无效 ID（0 或解析失败）

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

    // 歌手：优先 ARTIST（含 & 分隔的多歌手），回退 AARTIST（通常是英文名）。
    // 原始字段使用 `&` 分割，统一按 `&` 拆分后再用设置中的分隔符拼接，
    // 避免前端展示形式在不同平台间出现割裂感。
    let raw_artist = song["ARTIST"]
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

    // 拆分时忽略空白与空字符串；找不到分隔符时退化为单元素数组。
    let artists: Vec<String> = if raw_artist.contains('&') {
        raw_artist
            .split('&')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    } else if raw_artist.is_empty() {
        Vec::new()
    } else {
        vec![raw_artist]
    };
    let artist = artists.join(artist_separator);

    // 专辑
    let album = song["ALBUM"].as_str().unwrap_or("").to_string();

    // 时长（秒），用于前端展示。兼容字符串和数字两种类型：
    // 歌单接口返回字符串，搜索接口返回数字。
    let duration = match song["DURATION"].clone() {
        Value::String(s) => s.parse::<u64>().unwrap_or(0),
        Value::Number(n) => n.as_u64().unwrap_or(0),
        _ => 0,
    };

    // 封面 URL：优先使用歌曲对象中已有的 `albumpic` 字段（歌单接口返回），
    // 如果不存在则为空字符串，由前端按需调用 fetch_cover 获取。
    // 这样避免在已有封面链接时重复请求封面接口，减少耗时。
    let cover_url = song["albumpic"].as_str().unwrap_or("").to_string();

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

//! 酷我音乐歌曲解析与品质构建模块。
//!
//! 将酷我搜索接口返回的原始歌曲 JSON 转换为统一格式的歌曲信息对象，
//! 包括基础字段（ID、mid、标题、歌手、专辑、封面）和可用品质列表。
//!
//! 关键点：
//! - 酷我的 `MUSICRID` 形如 `MUSIC_6802907`，需去掉 `MUSIC_` 前缀转为数字 ID。
//! - 酷我没有字符串 mid，前端 `mid` 字段填数字 ID 的字符串形式。
//! - 品质从 `N_MINFO`（优先）或 `MINFO` 字段解析，按 `bitrate` → 品质标签映射。
//! - 封面取自完整图片地址或 `web_*pic_short` 路径，短路径统一使用 300×300 分辨率。
//! - 酷我接口的 `ARTIST` 字段本身使用 `&` 作为歌手分隔符；解析后按用户设置的
//!   `artistSeparator` 重新拼接，保持与其他平台一致的展示形式。

use regex::Regex;
use serde_json::{json, Value};

/// 将数字或数字字符串转为正整数 ID 字符串；无效值返回空字符串。
fn entity_id(value: &Value) -> String {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
        .filter(|id| *id > 0)
        .map(|id| id.to_string())
        .unwrap_or_default()
}

/// 将酷我图片短路径转换为 300×300 图片地址。
///
/// # 参数
/// - `value`: 形如 `120/40/64/2063938955.jpg` 的图片短路径。
/// - `directory`: 图片目录，歌手使用 `starheads`，专辑使用 `albumcover`。
///
/// # 返回
/// 完整的 HTTPS 图片地址；路径缺失或格式无效时返回空字符串。
fn picture_url(value: &Value, directory: &str) -> String {
    value
        .as_str()
        .and_then(|path| path.split_once('/'))
        .filter(|(size, path)| size.parse::<u32>().is_ok() && !path.is_empty())
        .map(|(_, path)| format!("https://img4.kuwo.cn/star/{}/300/{}", directory, path))
        .unwrap_or_default()
}

/// 按原始位置配对歌手名称和 ID。
///
/// # 参数
/// - `name`: 以 `&` 分隔的歌手名称。
/// - `item`: 包含 `allartistid` 或 `ALLARTISTID` 的原始条目。
///   单歌手条目缺少有效关联 ID 时，使用 `ARTISTID` 或 `artistid`。
///
/// # 返回
/// 按输入顺序排列的 `{ id, mid, name, coverUrl }` 列表，配对后过滤空白名称。
/// `id` 为数字 ID 字符串，缺失或无效时为空；`mid` 为空字符串。
/// `web_artistpic_short` 对应 `ARTISTID` 指定的歌手；单歌手条目可直接使用该图片。
pub(super) fn parse_artists(name: &str, item: &Value) -> Vec<Value> {
    let names: Vec<_> = name.split('&').collect();
    let all_ids = item["allartistid"]
        .as_str()
        .or_else(|| item["ALLARTISTID"].as_str())
        .unwrap_or("");
    let ids: Vec<_> = all_ids.split('&').collect();
    let single_id = entity_id(item.get("ARTISTID").unwrap_or(&item["artistid"]));
    let artist_cover = picture_url(&item["web_artistpic_short"], "starheads");
    names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            let mut id = ids
                .get(index)
                .and_then(|id| id.trim().parse::<u64>().ok())
                .filter(|id| *id > 0)
                .map(|id| id.to_string())
                .unwrap_or_default();
            if id.is_empty() && names.len() == 1 {
                id = single_id.clone();
            }
            let cover_url = if !id.is_empty()
                && (id == single_id || (single_id.is_empty() && names.len() == 1))
            {
                artist_cover.as_str()
            } else {
                ""
            };
            Some(json!({ "id": id, "mid": "", "name": name, "coverUrl": cover_url }))
        })
        .collect()
}

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
///   - `artists`: 歌手关联信息列表，每项包含 `id`、`mid`、`name` 和 `coverUrl`
///   - `album`: 专辑名
///   - `albumId`: 专辑数字 ID 的字符串形式
///   - `albumMid`: 空字符串
///   - `coverUrl`: 歌曲封面 URL，优先专辑图片，其次歌手图片
///   - `mediaMid`: 媒体文件标识（酷我用歌曲数字 ID）
///   - `qualities`: 可用品质列表，每项含 `quality`、`format`、`bitrate`、`size`、`filename`
/// - `None`：当歌曲缺少 `MUSICRID` 时返回 `None`，表示该歌曲无法解析或不可下载。
pub fn parse_song(song: &Value, artist_separator: &str) -> Option<Value> {
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

    let artists = parse_artists(&raw_artist, song);
    let artist = artists
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect::<Vec<_>>()
        .join(artist_separator);

    // 专辑
    // 专辑ID是song["ALBUMID"]，以字符串格式存储的专辑数字ID
    let album = song["ALBUM"].as_str().unwrap_or("").to_string();
    let ablum_id = entity_id(song.get("ALBUMID").unwrap_or(&song["albumid"]));

    // 时长（秒），用于前端展示。兼容字符串和数字两种类型：
    // 歌单接口返回字符串，搜索接口返回数字。
    let duration = match song["DURATION"].clone() {
        Value::String(s) => s.parse::<u64>().unwrap_or(0),
        Value::Number(n) => n.as_u64().unwrap_or(0),
        _ => 0,
    };

    let cover_url = song["albumpic"]
        .as_str()
        .filter(|url| !url.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let album_cover = picture_url(&song["web_albumpic_short"], "albumcover");
            if album_cover.is_empty() {
                picture_url(&song["web_artistpic_short"], "starheads")
            } else {
                album_cover
            }
        });

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
        "artists": artists,
        "album": album,
        "albumId": ablum_id,
        "albumMid": "",
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
pub fn build_qualities(info_str: &str) -> Vec<Value> {
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

/// 歌手、专辑详情接口使用小写字段，适配后交给统一歌曲解析器。
pub fn parse_detail_song(item: &Value, separator: &str) -> Option<Value> {
    let mut song = item.clone();
    for (target, source) in [
        ("SONGNAME", "name"),
        ("ARTIST", "artist"),
        ("ALBUM", "album"),
        ("DURATION", "duration"),
        ("MUSICRID", "musicrid"),
    ] {
        if let Some(value) = item.get(source) {
            song[target] = value.clone();
        }
    }
    parse_song(&song, separator)
}

#[cfg(test)]
mod artist_tests {
    use super::*;

    #[test]
    fn search_song_uses_artist_and_album_short_paths() {
        let mut raw = json!({
            "MUSICRID": "MUSIC_6802907",
            "ARTIST": "林俊杰", "ARTISTID": "1062", "allartistid": "1062",
            "web_albumpic_short": "120/55/84/1457166092.jpg",
            "web_artistpic_short": "120/40/64/2063938955.jpg"
        });
        let song = parse_song(&raw, "、").unwrap();
        assert_eq!(
            song["coverUrl"],
            "https://img4.kuwo.cn/star/albumcover/300/55/84/1457166092.jpg"
        );
        assert_eq!(
            song["artists"][0]["coverUrl"],
            "https://img4.kuwo.cn/star/starheads/300/40/64/2063938955.jpg"
        );

        raw["albumpic"] = json!("https://example.com/album.jpg");
        assert_eq!(parse_song(&raw, "、").unwrap()["coverUrl"], raw["albumpic"]);

        raw["albumpic"] = json!("");
        raw["web_albumpic_short"] = json!("");
        assert_eq!(
            parse_song(&raw, "、").unwrap()["coverUrl"],
            song["artists"][0]["coverUrl"]
        );

        raw["web_artistpic_short"] = json!("");
        assert_eq!(parse_song(&raw, "、").unwrap()["coverUrl"], "");
    }

    #[test]
    fn associates_artist_picture_with_its_artist_id() {
        let artists = parse_artists(
            "甲&乙",
            &json!({
                "allartistid": "336&123", "ARTISTID": "123",
                "web_artistpic_short": "120/40/64/2063938955.jpg"
            }),
        );
        assert_eq!(artists[0]["coverUrl"], "");
        assert_eq!(
            artists[1]["coverUrl"],
            "https://img4.kuwo.cn/star/starheads/300/40/64/2063938955.jpg"
        );
        assert_eq!(picture_url(&json!(""), "starheads"), "");
        assert_eq!(picture_url(&json!("invalid"), "starheads"), "");
    }

    #[test]
    fn keeps_artist_id_positions_when_names_or_ids_are_empty() {
        let artists = parse_artists("甲&&乙&丙", &json!({"allartistid": "123&999&&456"}));
        assert_eq!(
            artists,
            vec![
                json!({"id": "123", "mid": "", "name": "甲", "coverUrl": ""}),
                json!({"id": "", "mid": "", "name": "乙", "coverUrl": ""}),
                json!({"id": "456", "mid": "", "name": "丙", "coverUrl": ""})
            ]
        );
        let missing = parse_artists("甲&乙", &json!({"artistid": "123"}));
        assert!(missing.iter().all(|artist| artist["id"] == ""));
        assert_eq!(
            parse_artists("甲", &json!({"artistid": 123}))[0]["id"],
            "123"
        );
    }

    #[test]
    fn search_song_preserves_related_ids_and_custom_separator() {
        let song = parse_song(
            &json!({
                "MUSICRID": "MUSIC_228908", "ARTIST": "甲&乙",
                "allartistid": "336&123", "ALBUMID": "1293", "ALBUM": "叶惠美"
            }),
            "、",
        )
        .unwrap();
        assert_eq!(song["artist"], "甲、乙");
        assert_eq!(song["artists"][1]["id"], "123");
        assert_eq!(song["albumId"], "1293");
        assert_eq!(song["albumMid"], "");
    }
    #[test]
    fn parses_artist_song_without_id() {
        let song = parse_detail_song(
            &json!({
                "musicrid": "228908",
                "name": "晴天",
                "artist": "周杰伦",
                "album": "叶惠美",
                "albumid": 1293,
                "artistid": "336",
                "duration": "269",
                "web_albumpic_short": "120/test.jpg",
                "MINFO": "level:p,bitrate:320,format:mp3,size:10.29Mb"
            }),
            " / ",
        )
        .unwrap();
        assert_eq!(song["id"], 228908);
        assert_eq!(song["title"], "晴天");
        assert_eq!(song["album"], "叶惠美");
        assert_eq!(song["albumId"], "1293");
        assert_eq!(song["artists"][0]["id"], "336");
        assert_eq!(song["duration"], 269);
        assert_eq!(
            song["coverUrl"],
            "https://img4.kuwo.cn/star/albumcover/300/test.jpg"
        );
        assert!(!song["qualities"].as_array().unwrap().is_empty());
    }
}

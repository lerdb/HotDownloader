//! 歌曲解析与品质构建模块。
//!
//! 该模块提供从 QQ 音乐原始歌曲 JSON 数据中提取统一格式歌曲信息的功能，
//! 包括基础字段（ID、mid、标题、歌手、专辑、封面）和可用品质列表的构建。
//! 供搜索、歌单等接口复用。

use serde_json::{json, Value};

/// 将数字或数字字符串转为正整数 ID 字符串；无效值返回空字符串。
fn entity_id(value: &Value) -> String {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse::<u64>().ok()))
        .filter(|id| *id > 0)
        .map(|id| id.to_string())
        .unwrap_or_default()
}

/// 解析歌手关联信息，保留数字 ID、MID 和名称。
///
/// # 参数
/// - `value`: 歌手数组，字段使用 `id` / `mid` / `name` 或对应的 `singer_` 前缀形式。
///
/// # 返回
/// 按输入顺序排列的 `{ id, mid, name }` 列表，过滤名称缺失或空白的条目。
/// 输入不是数组时返回空列表。
pub(super) fn parse_artists(value: &Value) -> Vec<Value> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item["name"]
                        .as_str()
                        .or_else(|| item["singer_name"].as_str())?;
                    if name.trim().is_empty() {
                        return None;
                    }
                    let id = entity_id(item.get("id").unwrap_or(&item["singer_id"]));
                    let mid = item["mid"]
                        .as_str()
                        .or_else(|| item["singer_mid"].as_str())
                        .unwrap_or("");
                    Some(json!({ "id": id, "mid": mid, "name": name }))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 通用歌曲解析函数。
///
/// 将搜索或歌单接口返回的原始歌曲 JSON 对象转换为前端需要的统一格式。
/// 主要提取歌曲 ID、mid、标题、歌手、专辑、封面 URL、媒体 mid 以及可用品质列表。
///
/// # 参数
/// - `song`: 歌曲原始 JSON 对象（搜索或歌单接口中的一项）。
/// - `artist_separator`: 多名歌手之间的连接字符串（来自 `artistSeparator` 设置）。
///
/// # 返回
/// - `Some(Value)`：成功解析的歌曲信息 JSON 对象，包含以下字段：
///   - `id`: 数字歌曲 ID（用于歌词等需要数字 ID 的接口）
///   - `mid`: 歌曲唯一标识（字符串 mid）
///   - `title`: 歌曲标题（优先取 `name` 字段，若为空则取 `title` 字段）
///   - `artist`: 歌手名（多个歌手用 `artist_separator` 连接）
///   - `artists`: 歌手关联信息列表，每项包含 `id`、`mid` 和 `name`
///   - `album`: 专辑名
///   - `albumId`: 专辑数字 ID 的字符串形式
///   - `albumMid`: 专辑 MID
///   - `coverUrl`: 封面图片 URL（优先专辑封面，其次歌手头像，均无则为空字符串）
///   - `mediaMid`: 媒体文件 mid（用于下载链接生成）
///   - `qualities`: 可用品质列表（由 [`build_qualities`] 生成）
/// - `None`：当歌曲缺少 `mid` 或 `media_mid` 时返回 `None`，表示该歌曲无法解析或不可下载。
pub fn parse_song(song: &Value, artist_separator: &str) -> Option<Value> {
    // 歌曲唯一标识（使用 mid），缺失则跳过该歌曲
    let mid = song["mid"].as_str().unwrap_or("").to_string();
    if mid.is_empty() {
        return None;
    }

    // 数字歌曲 ID，用于歌词接口等需要数字 ID 的场景。
    // 兼容字段可能以数字或字符串形式出现。
    let song_id = song["id"]
        .as_u64()
        .or_else(|| song["id"].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0);

    // 媒体文件 mid，必须有该字段才能生成下载链接，否则跳过
    let media_mid = song["file"]["media_mid"].as_str().unwrap_or("").to_string();
    if media_mid.is_empty() {
        return None;
    }

    // 标题：优先使用 `name` 字段，若为空则尝试 `title` 字段，最终回退为空字符串
    let title = song["name"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            song["title"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();

    // 歌手关联信息与显示名称共用解析结果。
    let artists = parse_artists(&song["singer"]);
    let artist = artists
        .iter()
        .filter_map(|artist| artist["name"].as_str())
        .collect::<Vec<_>>()
        .join(artist_separator);

    // 专辑名
    let album_name = song["album"]["name"].as_str().unwrap_or("").to_string();

    // 封面信息：优先使用专辑 mid，否则使用第一个歌手的 mid
    let album_mid = song["album"]["mid"].as_str().unwrap_or("");
    let first_singer_mid = song["singer"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|s| s["mid"].as_str())
        .unwrap_or("");

    // 构建封面 URL。
    // 使用可变变量避免在 if-else 表达式中直接赋值可能带来的所有权问题。
    let mut cover_url = String::new();
    if !album_mid.is_empty() && album_mid != "空" {
        cover_url = format!(
            "https://y.gtimg.cn/music/photo_new/T002R500x500M000{}.jpg",
            album_mid
        );
    } else if !first_singer_mid.is_empty() {
        cover_url = format!(
            "https://y.gtimg.cn/music/photo_new/T001R500x500M000{}.jpg",
            first_singer_mid
        );
    }

    // 构建可用品质列表（基于 file 和 vs 字段）
    let qualities = build_qualities(&song["file"], &song["vs"]);

    Some(json!({
        "id": song_id,
        "mid": mid,
        "title": title,
        "artist": artist,
        "artists": artists,
        "album": album_name,
        "albumId": entity_id(&song["album"]["id"]),
        "albumMid": album_mid,
        "coverUrl": cover_url,
        "mediaMid": media_mid,
        "qualities": qualities
    }))
}

/// 根据歌曲的 `file` 和 `vs` 字段生成可用品质列表。
///
/// 品质列表包含标准品质（如各种比特率的 AAC、OGG、MP3、APE、FLAC、Hi-Res）
/// 以及特殊高品质（臻品全景声、臻品母带）。
/// 每个品质项包含品质标签、文件名（由前缀 + media_mid + 后缀构成）和文件大小。
///
/// # 参数
/// - `file`: 歌曲文件信息 JSON 对象，通常来自原始数据的 `file` 字段，
///   需包含 `media_mid` 以及各个品质对应的 `size_*` 字段，可能还包含 `size_new` 数组。
/// - `vs`: 特殊品质对应的验证字符串数组，通常来自原始数据的 `vs` 字段，
///   其中第 3 项用于臻品母带，第 4 项用于臻品全景声。
///
/// # 返回
/// 一个 `Vec<Value>`，每个元素为包含 `quality`、`filename`、`size` 字段的 JSON 对象。
/// 只返回文件大小大于 0 的品质项；特殊品质还需满足对应的 vs 字符串非空。
pub fn build_qualities(file: &Value, vs: &Value) -> Vec<Value> {
    let media_mid = file["media_mid"].as_str().unwrap_or("");
    let mut list: Vec<Value> = Vec::new();

    // 标准品质定义：(前端显示标签, 文件名前缀, 文件扩展名, file 中的大小字段名)
    let standard_qualities: Vec<(&str, &str, &str, &str)> = vec![
        ("48kaac", "C200", ".m4a", "size_48aac"),
        ("96kaac", "C400", ".m4a", "size_96aac"),
        ("192kaac", "C600", ".m4a", "size_192aac"),
        ("96kogg", "O4M0", ".mgg", "size_96ogg"),
        ("192kogg", "O6M0", ".mgg", "size_192ogg"),
        ("128kmp3", "M500", ".mp3", "size_128mp3"),
        ("320kmp3", "M800", ".mp3", "size_320mp3"),
        ("ape", "A000", ".ape", "size_ape"),
        ("flac", "F0M0", ".mflac", "size_flac"),
        ("hires", "RSM1", ".mflac", "size_hires"),
    ];

    // 遍历标准品质，若对应大小字段存在且 >0 则加入列表
    for (label, prefix, suffix, size_key) in &standard_qualities {
        let size = file[*size_key].as_u64().unwrap_or(0);
        if size > 0 {
            list.push(json!({
                "quality": label,
                "filename": format!("{}{}{}", prefix, media_mid, suffix),
                "size": size
            }));
        }
    }

    // 特殊品质处理：依赖 size_new 数组和 vs 数组
    // 顺序：臻品全景声 → 臻品全景声 5.1 → 臻品母带
    let size_new = file["size_new"].as_array();
    let vs_arr = vs.as_array();
    if let (Some(size_new), Some(vs_arr)) = (size_new, vs_arr) {
        // 提取 vs 数组中特定位置的字符串，若不存在则为空字符串
        let vs3 = vs_arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
        let vs4 = vs_arr.get(4).and_then(|v| v.as_str()).unwrap_or("");

        // 臻品全景声：size_new[1] 为文件大小，vs[4] 为验证字符串
        let size_panorama = size_new.get(1).and_then(|v| v.as_u64()).unwrap_or(0);
        if size_panorama > 0 && !vs4.is_empty() {
            list.push(json!({
                "quality": "臻品全景声",
                "filename": format!("Q0M0{}.mflac", vs4),
                "size": size_panorama
            }));
        }

        // 臻品全景声 5.1：size_new[2] 为文件大小，vs[4] 为验证字符串
        let size_panorama_51 = size_new.get(2).and_then(|v| v.as_u64()).unwrap_or(0);
        if size_panorama_51 > 0 && !vs4.is_empty() {
            list.push(json!({
                "quality": "臻品全景声 5.1",
                "filename": format!("Q0M1{}.mflac", vs4),
                "size": size_panorama_51
            }));
        }

        // 臻品母带：size_new[0] 为文件大小，vs[3] 为验证字符串
        let size_master = size_new.first().and_then(|v| v.as_u64()).unwrap_or(0);
        if size_master > 0 && !vs3.is_empty() {
            list.push(json!({
                "quality": "臻品母带",
                "filename": format!("AIM0{}.mflac", vs3),
                "size": size_master
            }));
        }
    }

    list
}

#[cfg(test)]
mod navigation_tests {
    use super::*;

    #[test]
    fn preserves_numeric_ids_and_mids_for_each_artist_and_album() {
        let song = parse_song(
            &json!({
                "mid": "songMid", "file": {"media_mid": "mediaMid"},
                "singer": [
                    {"id": 4558, "mid": "artistA", "name": "甲"},
                    {"id": "4944", "mid": "artistB", "name": "乙"}
                ],
                "album": {"id": 8217, "mid": "albumMid", "name": "专辑"}
            }),
            " / ",
        )
        .unwrap();
        assert_eq!(song["artist"], "甲 / 乙");
        assert_eq!(
            song["artists"][0],
            json!({"id": "4558", "mid": "artistA", "name": "甲"})
        );
        assert_eq!(song["artists"][1]["mid"], "artistB");
        assert_eq!(song["albumId"], "8217");
        assert_eq!(song["albumMid"], "albumMid");
    }

    #[test]
    fn missing_mid_is_not_replaced_with_numeric_id() {
        let artists = parse_artists(&json!([
            {"id": 4558, "mid": "", "name": "周杰伦"},
            {"singer_id": "4944", "singer_mid": "artistB", "singer_name": "乙"},
            {"id": 0, "name": "未知"}
        ]));
        assert_eq!(artists[0]["id"], "4558");
        assert_eq!(artists[0]["mid"], "");
        assert_eq!(artists[1]["mid"], "artistB");
        assert_eq!(artists[2]["id"], "");
    }
}

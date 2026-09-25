//! 分页获取专辑内的歌曲，复用统一歌曲解析器。
//!
//! 通过 QQ 音乐 `GetAlbumSongList` 接口按页拉取专辑歌曲，
//! 使用 `seen` 集合去重，最终返回专辑信息和歌曲列表的 JSON 字符串。

use crate::platforms::CLIENT;
use serde_json::{json, Value};

/// 分页获取指定专辑内的全部歌曲。
///
/// # 参数
/// - `separator`: 运行时提供的歌手名分隔符。
/// - `id`: 专辑 MID。
///
/// # 返回
/// - `Ok(String)`: 包含 `album` 和 `songs` 的 JSON 字符串。
/// - `Err(String)`: 参数无效、请求失败或分页异常。
pub async fn fetch_album_songs(separator: &str, id: String) -> Result<String, String> {
    // 专辑 MID 只允许 ASCII 字母和数字
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("无效的专辑 MID".into());
    }

    let mut songs = Vec::new();
    let mut begin = 0u64;
    // 用于按歌曲 mid 去重
    let mut seen = std::collections::HashSet::new();

    // 最多循环 1000 次，防止异常情况下无限翻页
    for _ in 0..1000 {
        // 构造分页请求体，每页固定 100 首
        let body = json!({
            "comm": super::search::mobile_comm(),
            "albumSonglist": {
                "method": "GetAlbumSongList",
                "module": "music.musichallAlbum.AlbumSongList",
                "param": {
                    "albumMid": id,
                    "albumID": 0,
                    "begin": begin,
                    "num": 100,
                    "order": 2
                }
            }
        });

        // 发送 POST 请求并解析 JSON
        let response = CLIENT
            .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
            .header("Referer", "https://y.qq.com")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("专辑请求失败: {}", e))?
            .error_for_status()
            .map_err(|e| format!("专辑请求失败: {}", e))?;
        let data: Value = response
            .json()
            .await
            .map_err(|e| format!("专辑解析失败: {}", e))?;

        // 顶层 code 和 albumSonglist 子请求 code 都必须为 0
        if data["code"] != 0 || data["albumSonglist"]["code"] != 0 {
            return Err("QQ 音乐专辑接口返回错误".into());
        }

        let data = &data["albumSonglist"]["data"];
        let items = data["songList"].as_array().ok_or("未找到专辑歌曲列表")?;

        // totalNum 兼容数字和字符串两种形式
        let total = data["totalNum"]
            .as_u64()
            .or_else(|| data["totalNum"].as_str().and_then(|s| s.parse().ok()))
            .ok_or("未找到专辑歌曲总数")?;

        // 如果本页为空但还没取完，说明接口返回异常
        if items.is_empty() && begin < total {
            return Err("专辑歌曲未完整返回，请重试".into());
        }

        // 记录去重前的数量，用于检测本页是否全是重复数据
        let before = seen.len();

        // 逐条解析歌曲，按 mid 去重后加入结果
        for item in items {
            let raw = &item["songInfo"];
            if seen.insert(raw["mid"].to_string()) {
                if let Some(song) = super::parser::parse_song(raw, separator) {
                    songs.push(song);
                }
            }
        }

        // 更新下一页的起始位置
        begin += items.len() as u64;

        // 如果本页非空但没有新增任何歌曲，说明接口返回了重复分页
        if !items.is_empty() && before == seen.len() {
            return Err("专辑接口返回重复分页，请重试".into());
        }

        // 已取完所有歌曲，从第一首歌中提取专辑名和歌手名，组装最终结果
        if begin >= total {
            let first = songs.first().cloned().unwrap_or(Value::Null);
            return Ok(json!({
                "album": {
                    "id": id,
                    "name": first["album"].as_str().unwrap_or(""),
                    "artist": first["artist"].as_str().unwrap_or(""),
                    "artists": first["artists"].as_array().cloned().unwrap_or_default(),
                    "coverUrl": format!("https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg", id),
                    "publishDate": "",
                    "songCount": total
                },
                "songs": songs
            })
            .to_string());
        }
    }

    // 循环次数达到上限仍未取完，视为异常
    Err("专辑分页超出限制".into())
}

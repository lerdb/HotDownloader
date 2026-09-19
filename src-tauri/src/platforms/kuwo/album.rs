//! 专辑信息与完整歌曲列表，适配字段后复用歌曲解析器。
//!
//! 通过酷我 `search.kuwo.cn` 接口按页拉取专辑歌曲，
//! 首屏解析专辑元信息，歌曲条目经字段适配后交给统一解析器处理。

use crate::utils::{filename::get_artist_separator, http::CLIENT};
use serde_json::{json, Value};
use tauri::AppHandle;

/// 从 JSON 值中提取数字，兼容数字和字符串，失败时返回 0。
pub(super) fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

/// 解析专辑元信息，返回前端需要的 JSON 对象。
///
/// # 参数
/// - `item`: 原始专辑数据。
/// - `separator`: 多位歌手之间的连接符。
///
/// # 返回
/// 解析后的专辑 JSON 对象。
pub(super) fn parse_album(item: &Value, separator: &str) -> Value {
    // 专辑 ID 可能叫 albumid 也可能叫 id
    let id = item
        .get("albumid")
        .or_else(|| item.get("id"))
        .unwrap_or(&Value::Null);

    // 封面优先取 hts_img，其次 img，最后回退到拼装的酷我封面
    let cover = item["hts_img"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| item["img"].as_str().filter(|s| !s.is_empty()))
        .map(str::to_owned)
        .unwrap_or_else(|| {
            item["pic"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| format!("https://img3.kuwo.cn/star/albumcover/{}", s))
                .unwrap_or_default()
        });

    json!({
        // ID 可能是字符串或数字，统一转成字符串
        "id": id.as_str().map(str::to_owned).unwrap_or_else(|| number(id).to_string()),
        "name": item["name"].as_str().unwrap_or(""),
        // 歌手名以 & 分隔，拆分后按分隔符重新拼接
        "artist": item["artist"]
            .as_str()
            .unwrap_or("")
            .split('&')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(separator),
        "coverUrl": cover.replace("http://", "https://"),
        "publishDate": item["pub"].as_str().unwrap_or(""),
        // 歌曲数优先取 songnum，回退到 musiccnt
        "songCount": number(item.get("songnum").unwrap_or(&item["musiccnt"]))
    })
}

/// 适配专辑歌曲字段后调用统一歌曲解析器。
///
/// # 参数
/// - `item`: 原始歌曲条目。
/// - `separator`: 歌手名连接符。
/// - `cover`: 专辑封面 URL。
///
/// # 返回
/// - `Some(Value)`: 解析后的歌曲对象。
/// - `None`: 解析失败。
fn parse_album_song(item: &Value, separator: &str, cover: &Value) -> Option<Value> {
    let mut song = super::parser::parse_detail_song(item, separator)?;
    // 专辑接口未提供歌曲封面，统一用专辑封面填充
    song["coverUrl"] = cover.clone();
    Some(song)
}

/// 分页获取专辑信息与完整歌曲列表。
///
/// # 参数
/// - `app`: Tauri 应用句柄，用于读取歌手名分隔符。
/// - `id`: 专辑 ID（纯数字）。
///
/// # 返回
/// - `Ok(String)`: 包含 `album` 和 `songs` 的 JSON 字符串。
/// - `Err(String)`: 参数无效、请求失败或分页异常。
pub(crate) async fn fetch_album_songs(app: &AppHandle, id: String) -> Result<String, String> {
    // 酷我专辑 ID 必须是正整数
    if id.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
        return Err("无效的专辑 ID".into());
    }

    let separator = get_artist_separator(app);
    let mut songs = Vec::new();
    let mut album = Value::Null;
    // 按歌曲 id 去重
    let mut seen = std::collections::HashSet::new();
    let mut received = 0;

    // 最多循环 1000 页，防止异常情况下无限翻页
    for page in 0..1000 {
        // 酷我搜索接口采用 GET + query 参数，分页 pn 从 0 开始
        let response = CLIENT
            .get("http://search.kuwo.cn/r.s")
            .query(&[
                ("pn", page.to_string()),
                ("rn", "100".into()),
                ("albumid", id.clone()),
                ("stype", "albuminfo".into()),
                ("sortby", "0".into()),
                ("alflac", "1".into()),
                ("show_copyright_off", "1".into()),
                ("pcmp4", "1".into()),
                ("encoding", "utf8".into()),
                ("plat", "pc".into()),
                ("vipver", "MUSIC_9.1.0".into()),
                ("newver", "1".into()),
                ("pcjson", "1".into()),
            ])
            .send()
            .await
            .map_err(|e| format!("专辑请求失败: {}", e))?
            .error_for_status()
            .map_err(|e| format!("专辑请求失败: {}", e))?;
        let data: Value = response
            .json()
            .await
            .map_err(|e| format!("专辑解析失败: {}", e))?;

        let items = data["musiclist"].as_array().ok_or("未找到专辑歌曲列表")?;

        // 首屏从顶层响应解析专辑元信息
        if album.is_null() {
            album = parse_album(&data, &separator);
        }

        // 总数兼容数字和字符串
        let total = data["songnum"]
            .as_u64()
            .or_else(|| data["songnum"].as_str().and_then(|s| s.parse().ok()))
            .ok_or("未找到专辑歌曲总数")?;

        // 本页为空但还没取完，说明接口返回异常
        if items.is_empty() && received < total {
            return Err("专辑歌曲未完整返回，请重试".into());
        }

        // 记录去重前数量，用于检测本页是否全是重复数据
        let before = seen.len();

        // 逐条按 id 去重并解析
        for item in items {
            let key = item["id"].to_string();
            if seen.insert(key) {
                if let Some(song) = parse_album_song(item, &separator, &album["coverUrl"]) {
                    songs.push(song);
                }
            }
        }

        received += items.len() as u64;

        // 本页非空但无新增，说明接口返回了重复分页
        if !items.is_empty() && before == seen.len() {
            return Err("专辑接口返回重复分页，请重试".into());
        }

        // 已取完所有歌曲，返回专辑和歌曲列表
        if received >= total {
            return Ok(json!({
                "album": album,
                "songs": songs
            })
            .to_string());
        }
    }

    // 循环次数达到上限仍未取完，视为异常
    Err("专辑分页超出限制".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证专辑歌曲字段适配后能被统一解析器正确解析。
    #[test]
    fn adapts_album_song_fields() {
        let song = parse_album_song(
            &json!({
                "id": "476991828",
                "name": "即兴曲",
                "artist": "甲&乙",
                "album": "即兴曲",
                "duration": "99",
                "MINFO": "level:p,bitrate:320,format:mp3,size:3.78Mb"
            }),
            " / ",
            &json!("https://example.com/cover.jpg"),
        )
        .unwrap();
        assert_eq!(song["title"], "即兴曲");
        assert_eq!(song["artist"], "甲 / 乙");
        assert_eq!(song["duration"], 99);
        assert_eq!(song["album"], "即兴曲");
        assert!(!song["qualities"].as_array().unwrap().is_empty());
    }
}

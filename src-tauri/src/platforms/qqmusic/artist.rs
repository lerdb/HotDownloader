//! 歌手搜索条目和歌曲、专辑分页。
//!
//! 提供歌手条目解析、专辑解析，以及歌手歌曲/专辑的分页获取。

use crate::utils::{filename::get_artist_separator, http::CLIENT};
use serde_json::{json, Value};
use tauri::AppHandle;

/// 从 JSON 值中提取 `u64`，兼容数字和字符串两种形式。
fn number(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

/// 解析歌手条目，返回前端需要的 JSON 对象。
///
/// # 参数
/// - `item`: 原始歌手条目。
///
/// # 返回
/// - `Some(Value)`: 解析后的歌手对象。
/// - `None`: 缺少有效的 `singerMID`。
pub(super) fn parse_artist(item: &Value) -> Option<Value> {
    let id = item["singerMID"].as_str().filter(|s| !s.is_empty())?;
    Some(json!({
        "id": id,
        "name": item["singerName"].as_str().unwrap_or(""),
        // 封面统一升级为 https
        "coverUrl": item["singerPic"].as_str().unwrap_or("").replace("http://", "https://"),
        "songCount": number(&item["songNum"]).unwrap_or(0),
        "albumCount": number(&item["albumNum"]).unwrap_or(0),
        "alias": "",
        "region": ""
    }))
}

/// 解析专辑条目，返回前端需要的 JSON 对象。
///
/// # 参数
/// - `item`: 原始专辑条目。
/// - `separator`: 多位歌手之间的连接符。
///
/// # 返回
/// - `Some(Value)`: 解析后的专辑对象。
/// - `None`: 缺少有效的 `album_mid`。
fn parse_album(item: &Value, separator: &str) -> Option<Value> {
    let id = item["album_mid"].as_str().filter(|s| !s.is_empty())?;

    let artists = super::parser::parse_artists(&item["singers"]);
    let names: Vec<&str> = artists
        .iter()
        .filter_map(|artist| artist["name"].as_str())
        .collect();

    // 数组为空时回退到顶层 singer_name
    let artist = if names.is_empty() {
        item["singer_name"].as_str().unwrap_or("").to_owned()
    } else {
        names.join(separator)
    };

    Some(json!({
        "id": id,
        "name": item["album_name"].as_str().unwrap_or(""),
        "artist": artist,
        "artists": artists,
        "coverUrl": format!("https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg", id),
        "publishDate": item["pub_time"].as_str().unwrap_or(""),
        "songCount": number(&item["latest_song"]["song_count"]).unwrap_or(0)
    }))
}

/// 调用 QQ 音乐统一接口，返回子请求的 `data` 字段。
///
/// # 参数
/// - `module`: 接口模块名。
/// - `method`: 接口方法名。
/// - `param`: 接口参数。
///
/// # 返回
/// - `Ok(Value)`: 接口返回的 `data` 字段。
/// - `Err(String)`: 网络、解析或业务错误。
async fn api_call(module: &str, method: &str, param: Value) -> Result<Value, String> {
    // 子请求在响应中的 key
    let req_key = format!("{module}.{method}");

    // 统一使用 mobile_comm() 作为公共参数
    let body = json!({
        "comm": super::search::mobile_comm(),
        format!("{module}.{method}"): {
            "module": module,
            "method": method,
            "param": param,
        }
    });

    // 发送请求并读取 JSON
    let response = CLIENT
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Referer", "https://y.qq.com")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("歌手请求失败: {}", e))?
        .error_for_status()
        .map_err(|e| format!("歌手请求失败: {}", e))?;
    let data: Value = response
        .json()
        .await
        .map_err(|e| format!("歌手响应解析失败: {}", e))?;

    // 顶层 code 和子请求 code 都必须为 0
    if data["code"] != 0 || data[&req_key]["code"] != 0 {
        return Err(format!(
            "QQ 音乐歌手接口错误: {} / {}",
            data["code"], data[&req_key]["code"]
        ));
    }
    Ok(data[&req_key]["data"].clone())
}

/// 构造并发送歌手歌曲或专辑的分页请求。
///
/// # 参数
/// - `id`: 歌手 MID。
/// - `page`: 页码，从 1 开始。
/// - `limit`: 每页数量。
/// - `albums`: `true` 请求专辑，`false` 请求歌曲。
///
/// # 返回
/// - `Ok(Value)`: 接口返回的 `data` 字段。
/// - `Err(String)`: 参数无效或请求失败。
async fn request(id: &str, page: u32, limit: u32, albums: bool) -> Result<Value, String> {
    // 歌手 MID 只允许 ASCII 字母和数字
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("无效的歌手 MID".into());
    }

    let begin = u64::from(page.saturating_sub(1)) * u64::from(limit);

    // 根据 albums 选择模块、方法和参数
    let (module, method, param) = if albums {
        (
            "music.web_singer_info_svr",
            "get_singer_album",
            json!({
                "singermid": id,
                "order": "time",
                "begin": begin,
                "num": limit,
                "exstatus": 1
            }),
        )
    } else {
        (
            "musichall.song_list_server",
            "GetSingerSongList",
            json!({
                "singerMid": id,
                "order": 1,
                "begin": begin,
                "num": limit
            }),
        )
    };

    // 调用统一接口公共函数，自动构建请求体、发送请求并解析响应
    api_call(module, method, param).await
}

/// 分页获取歌手歌曲列表，返回 JSON 字符串。
///
/// # 参数
/// - `app`: Tauri 应用句柄，用于读取歌手名分隔符。
/// - `id`: 歌手 MID。
/// - `page`: 页码，从 1 开始。
/// - `limit`: 每页数量。
///
/// # 返回
/// - `Ok(String)`: 包含 `songs`、`total`、`has_more` 的 JSON。
/// - `Err(String)`: 请求或解析失败。
pub(crate) async fn fetch_artist_songs(
    app: &AppHandle,
    id: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = request(&id, page, limit, false).await?;

    let items = data["songList"].as_array().ok_or("未找到歌手歌曲列表")?;
    let total = number(&data["totalNum"]).ok_or("未找到歌手歌曲总数")?;
    let separator = get_artist_separator(app);

    // 逐条解析歌曲信息
    let songs: Vec<Value> = items
        .iter()
        .filter_map(|item| super::parser::parse_song(&item["songInfo"], &separator))
        .collect();

    // 已加载数量 = (page - 1) * limit + 本页实际返回数量
    let has_more = !items.is_empty()
        && u64::from(page.saturating_sub(1)) * u64::from(limit) + (items.len() as u64) < total;

    Ok(json!({
        "songs": songs,
        "total": total,
        "has_more": has_more
    })
    .to_string())
}

/// 分页获取歌手专辑列表，返回 JSON 字符串。
///
/// # 参数
/// - `app`: Tauri 应用句柄，用于读取歌手名分隔符。
/// - `id`: 歌手 MID。
/// - `page`: 页码，从 1 开始。
/// - `limit`: 每页数量。
///
/// # 返回
/// - `Ok(String)`: 包含 `albums`、`total`、`has_more`、`name` 的 JSON。
/// - `Err(String)`: 请求或解析失败。
pub(crate) async fn fetch_artist_albums(
    app: &AppHandle,
    id: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = request(&id, page, limit, true).await?;

    let items = data["list"].as_array().ok_or("未找到歌手专辑列表")?;
    let total = number(&data["total"]).ok_or("未找到歌手专辑总数")?;
    let separator = get_artist_separator(app);

    // 逐条解析专辑信息
    let albums: Vec<Value> = items
        .iter()
        .filter_map(|item| parse_album(item, &separator))
        .collect();

    // 已加载数量 = (page - 1) * limit + 本页实际返回数量
    let has_more = !items.is_empty()
        && u64::from(page.saturating_sub(1)) * u64::from(limit) + (items.len() as u64) < total;

    Ok(json!({
        "albums": albums,
        "total": total,
        "has_more": has_more,
        "name": data["singer_name"].as_str().unwrap_or("")
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证歌手和专辑导航使用 MID 作为唯一标识。
    #[test]
    fn uses_mid_for_artist_and_album_navigation() {
        let artist = parse_artist(&json!({
            "singerID": 4558,
            "singerMID": "0025NhlN2yWrP4",
            "singerName": "周杰伦",
            "songNum": 1012,
            "albumNum": 43
        }))
        .unwrap();
        assert_eq!(artist["id"], "0025NhlN2yWrP4");

        let album = parse_album(
            &json!({
                "albumid": 123,
                "album_mid": "albumMid",
                "album_name": "专辑",
                "singers": [
                    { "singer_name": "甲" },
                    { "singer_name": "乙" }
                ],
                "latest_song": { "song_count": 13 }
            }),
            " / ",
        )
        .unwrap();
        assert_eq!(album["id"], "albumMid");
        assert_eq!(album["artist"], "甲 / 乙");
        assert_eq!(album["songCount"], 13);
    }
}

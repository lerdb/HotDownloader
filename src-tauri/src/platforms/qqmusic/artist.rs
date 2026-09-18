//! 歌手搜索条目和歌曲、专辑分页。
use crate::utils::{filename::get_artist_separator, http::CLIENT};
use serde_json::{json, Value};
use tauri::AppHandle;

fn number(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

pub(super) fn parse_artist(item: &Value) -> Option<Value> {
    let id = item["singerMID"].as_str().filter(|s| !s.is_empty())?;
    Some(json!({
        "id": id,
        "name": item["singerName"].as_str().unwrap_or(""),
        "coverUrl": item["singerPic"].as_str().unwrap_or("").replace("http://", "https://"),
        "songCount": number(&item["songNum"]).unwrap_or(0),
        "albumCount": number(&item["albumNum"]).unwrap_or(0),
        "alias": "",
        "region": ""
    }))
}

fn parse_album(item: &Value, separator: &str) -> Option<Value> {
    let id = item["album_mid"].as_str().filter(|s| !s.is_empty())?;
    let names: Vec<&str> = item["singers"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|s| s["singer_name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    let artist = if names.is_empty() {
        item["singer_name"].as_str().unwrap_or("").to_owned()
    } else {
        names.join(separator)
    };
    Some(json!({
        "id": id,
        "name": item["album_name"].as_str().unwrap_or(""),
        "artist": artist,
        "coverUrl": format!("https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg", id),
        "publishDate": item["pub_time"].as_str().unwrap_or(""),
        "songCount": number(&item["latest_song"]["song_count"]).unwrap_or(0)
    }))
}

async fn request(id: &str, page: u32, limit: u32, albums: bool) -> Result<Value, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("无效的歌手 MID".into());
    }
    let begin = u64::from(page.saturating_sub(1)) * u64::from(limit);
    let (key, body) = if albums {
        (
            "singerAlbum",
            json!({
                "comm": {
                    "ct": 24,
                    "cv": 0
                },
                "singerAlbum": {
                    "module": "music.web_singer_info_svr",
                    "method": "get_singer_album",
                    "param": {
                        "singermid": id,
                        "order": "time",
                        "begin": begin,
                        "num": limit,
                        "exstatus": 1
                    }
                }
            }),
        )
    } else {
        (
            "singerSongList",
            json!({
                "comm": super::search::mobile_comm(),
                "singerSongList": {
                    "module": "musichall.song_list_server",
                    "method": "GetSingerSongList",
                    "param": {
                        "singerMid": id,
                        "order": 1,
                        "begin": begin,
                        "num": limit
                    }
                }
            }),
        )
    };
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
    if data["code"] != 0 || data[key]["code"] != 0 {
        return Err(format!(
            "QQ 音乐歌手接口错误: {} / {}",
            data["code"], data[key]["code"]
        ));
    }
    Ok(data[key]["data"].clone())
}

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
    let songs: Vec<Value> = items
        .iter()
        .filter_map(|item| super::parser::parse_song(&item["songInfo"], &separator))
        .collect();
    let has_more = !items.is_empty()
        && u64::from(page.saturating_sub(1)) * u64::from(limit) + (items.len() as u64) < total;
    Ok(json!({
        "songs": songs,
        "total": total,
        "has_more": has_more
    })
    .to_string())
}

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
    let albums: Vec<Value> = items
        .iter()
        .filter_map(|item| parse_album(item, &separator))
        .collect();
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
                    {
                        "singer_name": "甲"
                    },
                    {
                        "singer_name": "乙"
                    }
                ],
                "latest_song": {
                    "song_count": 13
                }
            }),
            " / ",
        )
        .unwrap();
        assert_eq!(album["id"], "albumMid");
        assert_eq!(album["artist"], "甲 / 乙");
        assert_eq!(album["songCount"], 13);
    }
}

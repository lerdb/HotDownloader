//! 分页获取专辑内的歌曲，复用统一歌曲解析器。
use crate::utils::{filename::get_artist_separator, http::CLIENT};
use serde_json::{json, Value};
use tauri::AppHandle;

pub(crate) async fn fetch_album_songs(app: &AppHandle, id: String) -> Result<String, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("无效的专辑 MID".into());
    }
    let separator = get_artist_separator(app);
    let mut songs = Vec::new();
    let mut begin = 0u64;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..1000 {
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
        if data["code"] != 0 || data["albumSonglist"]["code"] != 0 {
            return Err("QQ 音乐专辑接口返回错误".into());
        }
        let data = &data["albumSonglist"]["data"];
        let items = data["songList"].as_array().ok_or("未找到专辑歌曲列表")?;
        let total = data["totalNum"]
            .as_u64()
            .or_else(|| data["totalNum"].as_str().and_then(|s| s.parse().ok()))
            .ok_or("未找到专辑歌曲总数")?;
        if items.is_empty() && begin < total {
            return Err("专辑歌曲未完整返回，请重试".into());
        }
        let before = seen.len();
        for item in items {
            let raw = &item["songInfo"];
            if seen.insert(raw["mid"].to_string()) {
                if let Some(song) = super::parser::parse_song(raw, &separator) {
                    songs.push(song);
                }
            }
        }
        begin += items.len() as u64;
        if !items.is_empty() && before == seen.len() {
            return Err("专辑接口返回重复分页，请重试".into());
        }
        if begin >= total {
            let first = songs.first().cloned().unwrap_or(Value::Null);
            return Ok(json!({
                "album": {
                    "id": id,
                    "name": first["album"].as_str().unwrap_or(""),
                    "artist": first["artist"].as_str().unwrap_or(""),
                    "coverUrl": format!("https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg", id),
                    "publishDate": "",
                    "songCount": total
                },
                "songs": songs
            }).to_string());
        }
    }
    Err("专辑分页超出限制".into())
}

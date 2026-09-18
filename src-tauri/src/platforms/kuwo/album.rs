//! 专辑信息与完整歌曲列表，适配字段后复用歌曲解析器。
use crate::utils::{filename::get_artist_separator, http::CLIENT};
use serde_json::{json, Value};
use tauri::AppHandle;

pub(super) fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

pub(super) fn parse_album(item: &Value, separator: &str) -> Value {
    let id = item
        .get("albumid")
        .or_else(|| item.get("id"))
        .unwrap_or(&Value::Null);
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
        "id": id.as_str().map(str::to_owned).unwrap_or_else(|| number(id).to_string()),
        "name": item["name"].as_str().unwrap_or(""),
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
        "songCount": number(item.get("songnum").unwrap_or(&item["musiccnt"]))
    })
}

fn parse_album_song(item: &Value, separator: &str, cover: &Value) -> Option<Value> {
    let mut song = super::parser::parse_detail_song(item, separator)?;
    song["coverUrl"] = cover.clone();
    Some(song)
}

pub(crate) async fn fetch_album_songs(app: &AppHandle, id: String) -> Result<String, String> {
    if id.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
        return Err("无效的专辑 ID".into());
    }
    let separator = get_artist_separator(app);
    let mut songs = Vec::new();
    let mut album = Value::Null;
    let mut seen = std::collections::HashSet::new();
    let mut received = 0;
    for page in 0..1000 {
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
        if album.is_null() {
            album = parse_album(&data, &separator);
        }
        let total = data["songnum"]
            .as_u64()
            .or_else(|| data["songnum"].as_str().and_then(|s| s.parse().ok()))
            .ok_or("未找到专辑歌曲总数")?;
        if items.is_empty() && received < total {
            return Err("专辑歌曲未完整返回，请重试".into());
        }
        let before = seen.len();
        for item in items {
            let key = item["id"].to_string();
            if seen.insert(key) {
                if let Some(song) = parse_album_song(item, &separator, &album["coverUrl"]) {
                    songs.push(song);
                }
            }
        }
        received += items.len() as u64;
        if !items.is_empty() && before == seen.len() {
            return Err("专辑接口返回重复分页，请重试".into());
        }
        if received >= total {
            return Ok(json!({
                "album": album,
                "songs": songs
            })
            .to_string());
        }
    }
    Err("专辑分页超出限制".into())
}

#[cfg(test)]
mod tests {
    use super::*;
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

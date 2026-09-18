//! 歌手信息及歌曲、专辑分页。
use super::album::{number, parse_album};
use crate::utils::{filename::get_artist_separator, http::CLIENT};
use serde_json::{json, Value};
use tauri::AppHandle;

pub(super) fn parse_artist(item: &Value) -> Option<Value> {
    let id = number(&item["ARTISTID"]);
    if id == 0 {
        return None;
    }
    let cover = item["hts_PICPATH"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            item["PICPATH"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| format!("https://img1.kuwo.cn/star/starheads/{}", s))
                .unwrap_or_default()
        });
    Some(json!({
        "id": id.to_string(),
        "name": item["ARTIST"].as_str().unwrap_or(""),
        "alias": item["AARTIST"].as_str().unwrap_or(""),
        "region": item["COUNTRY"].as_str().unwrap_or(""),
        "coverUrl": cover.replace("http://", "https://"),
        "songCount": number(&item["SONGNUM"]),
        "albumCount": number(&item["ALBUMNUM"])
    }))
}

async fn request(id: &str, page: u32, limit: u32, albums: bool) -> Result<Value, String> {
    if id.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
        return Err("无效的歌手 ID".into());
    }
    let response = CLIENT
        .get("http://search.kuwo.cn/r.s")
        .query(&[
            ("artistid", id.to_owned()),
            ("pn", page.saturating_sub(1).to_string()),
            ("rn", limit.to_string()),
            (
                "stype",
                if albums { "albumlist" } else { "artist2music" }.into(),
            ),
            ("sortby", if albums { "1" } else { "0" }.into()),
            ("alflac", "1".into()),
            ("show_copyright_off", "1".into()),
            ("pcmp4", "1".into()),
            ("encoding", "utf8".into()),
            ("plat", "pc".into()),
            ("thost", "search.kuwo.cn".into()),
            (
                "vipver",
                if albums {
                    "MUSIC_9.1.1.2_BCS2"
                } else {
                    "MUSIC_9.1.0"
                }
                .into(),
            ),
            ("devid", "38668888".into()),
            ("newver", "1".into()),
            ("pcjson", "1".into()),
        ])
        .send()
        .await
        .map_err(|e| format!("歌手请求失败: {}", e))?
        .error_for_status()
        .map_err(|e| format!("歌手请求失败: {}", e))?;
    response
        .json()
        .await
        .map_err(|e| format!("歌手响应解析失败: {}", e))
}

fn pagination(data: &Value, page: u32, limit: u32, returned: usize) -> Result<(u64, bool), String> {
    let total = data["total"]
        .as_u64()
        .or_else(|| data["total"].as_str().and_then(|s| s.parse().ok()))
        .ok_or("未找到歌手列表总数")?;
    Ok((
        total,
        returned > 0
            && u64::from(page.saturating_sub(1)) * u64::from(limit) + (returned as u64) < total,
    ))
}

pub(crate) async fn fetch_artist_songs(
    app: &AppHandle,
    id: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = request(&id, page, limit, false).await?;
    let items = data["musiclist"].as_array().ok_or("未找到歌手歌曲列表")?;
    let (total, has_more) = pagination(&data, page, limit, items.len())?;
    let separator = get_artist_separator(app);
    let songs: Vec<Value> = items
        .iter()
        .filter_map(|item| super::parser::parse_detail_song(item, &separator))
        .collect();
    Ok(json!({
        "songs": songs,
        "total": total,
        "has_more": has_more,
        "name": data["artist"].as_str().unwrap_or("")
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
    let items = data["albumlist"].as_array().ok_or("未找到歌手专辑列表")?;
    let (total, has_more) = pagination(&data, page, limit, items.len())?;
    let separator = get_artist_separator(app);
    let albums: Vec<Value> = items
        .iter()
        .map(|item| parse_album(item, &separator))
        .collect();
    Ok(json!({
        "albums": albums,
        "total": total,
        "has_more": has_more
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_artist_and_pagination() {
        let artist = parse_artist(&json!({
            "ARTISTID": "336",
            "ARTIST": "周杰伦",
            "SONGNUM": "1709",
            "ALBUMNUM": "45",
            "PICPATH": "240/test.jpg"
        }))
        .unwrap();
        assert_eq!(artist["id"], "336");
        assert_eq!(artist["songCount"], 1709);
        assert_eq!(
            artist["coverUrl"],
            "https://img1.kuwo.cn/star/starheads/240/test.jpg"
        );
        assert_eq!(
            pagination(
                &json!({
                    "total": "45"
                }),
                2,
                20,
                20
            )
            .unwrap(),
            (45, true)
        );
        assert_eq!(
            pagination(
                &json!({
                    "total": 45
                }),
                3,
                20,
                5
            )
            .unwrap(),
            (45, false)
        );
        assert!(
            !pagination(
                &json!({
                    "total": 45
                }),
                2,
                20,
                0
            )
            .unwrap()
            .1
        );
    }
}

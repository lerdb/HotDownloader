//! 歌手信息及歌曲、专辑分页。
//!
//! 通过酷我 `search.kuwo.cn` 接口解析歌手条目，
//! 并按页获取歌手的歌曲或专辑列表。
//! 专辑解析复用 [`super::album::parse_album`]，歌曲解析复用 [`super::parser::parse_detail_song`]。

use super::album::{number, parse_album};
use crate::platforms::CLIENT;
use serde_json::{json, Value};

/// 解析歌手条目，返回前端需要的 JSON 对象。
///
/// # 参数
/// - `item`: 原始歌手条目。
///
/// # 返回
/// - `Some(Value)`: 解析后的歌手对象。
/// - `None`: `ARTISTID` 无效（为 0）。
pub(super) fn parse_artist(item: &Value) -> Option<Value> {
    let id = number(&item["ARTISTID"]);
    if id == 0 {
        return None;
    }

    // 头像优先取 hts_PICPATH，回退到拼装的酷我头像地址
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

/// 请求歌手歌曲或专辑分页数据，返回原始 JSON。
///
/// # 参数
/// - `id`: 歌手 ID（纯数字）。
/// - `page`: 页码，从 1 开始。
/// - `limit`: 每页数量。
/// - `albums`: `true` 请求专辑，`false` 请求歌曲。
///
/// # 返回
/// - `Ok(Value)`: 接口返回的原始 JSON。
/// - `Err(String)`: 参数无效或请求失败。
async fn request(id: &str, page: u32, limit: u32, albums: bool) -> Result<Value, String> {
    // 酷我歌手 ID 必须是正整数
    if id.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
        return Err("无效的歌手 ID".into());
    }

    // 酷我分页 pn 从 0 开始，因此用 page - 1
    let response = CLIENT
        .get("http://search.kuwo.cn/r.s")
        .query(&[
            ("artistid", id.to_owned()),
            ("pn", page.saturating_sub(1).to_string()),
            ("rn", limit.to_string()),
            // 根据 albums 切换请求类型
            (
                "stype",
                if albums { "albumlist" } else { "artist2music" }.into(),
            ),
            // 专辑按时间排序（1），歌曲按默认排序（0）
            ("sortby", if albums { "1" } else { "0" }.into()),
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
        .map_err(|e| format!("歌手请求失败: {}", e))?
        .error_for_status()
        .map_err(|e| format!("歌手请求失败: {}", e))?;

    response
        .json()
        .await
        .map_err(|e| format!("歌手响应解析失败: {}", e))
}

/// 从响应中提取总数并计算是否还有下一页。
///
/// # 参数
/// - `data`: 原始响应 JSON。
/// - `page`: 当前页码，从 1 开始。
/// - `limit`: 每页数量。
/// - `returned`: 本页实际返回的条目数。
///
/// # 返回
/// - `Ok((total, has_more))`: 总数与是否还有下一页。
/// - `Err(String)`: 缺少 `total` 字段。
fn pagination(data: &Value, page: u32, limit: u32, returned: usize) -> Result<(u64, bool), String> {
    // total 兼容数字和字符串
    let total = data["total"]
        .as_u64()
        .or_else(|| data["total"].as_str().and_then(|s| s.parse().ok()))
        .ok_or("未找到歌手列表总数")?;

    // 已加载数量 = (page - 1) * limit + 本页实际返回数量
    Ok((
        total,
        returned > 0
            && u64::from(page.saturating_sub(1)) * u64::from(limit) + (returned as u64) < total,
    ))
}

/// 分页获取歌手歌曲列表，返回 JSON 字符串。
///
/// # 参数
/// - `separator`: 运行时提供的歌手名分隔符。
/// - `id`: 歌手 ID。
/// - `page`: 页码，从 1 开始。
/// - `limit`: 每页数量。
///
/// # 返回
/// - `Ok(String)`: 包含 `songs`、`total`、`has_more`、`name` 的 JSON。
/// - `Err(String)`: 请求或解析失败。
pub async fn fetch_artist_songs(
    separator: &str,
    id: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = request(&id, page, limit, false).await?;

    let items = data["musiclist"].as_array().ok_or("未找到歌手歌曲列表")?;
    let (total, has_more) = pagination(&data, page, limit, items.len())?;

    // 逐条解析歌曲信息
    let songs: Vec<Value> = items
        .iter()
        .filter_map(|item| super::parser::parse_detail_song(item, separator))
        .collect();

    Ok(json!({
        "songs": songs,
        "total": total,
        "has_more": has_more,
        "name": data["artist"].as_str().unwrap_or("")
    })
    .to_string())
}

/// 分页获取歌手专辑列表，返回 JSON 字符串。
///
/// # 参数
/// - `separator`: 运行时提供的歌手名分隔符。
/// - `id`: 歌手 ID。
/// - `page`: 页码，从 1 开始。
/// - `limit`: 每页数量。
///
/// # 返回
/// - `Ok(String)`: 包含 `albums`、`total`、`has_more` 的 JSON。
/// - `Err(String)`: 请求或解析失败。
pub async fn fetch_artist_albums(
    separator: &str,
    id: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = request(&id, page, limit, true).await?;

    let items = data["albumlist"].as_array().ok_or("未找到歌手专辑列表")?;
    let (total, has_more) = pagination(&data, page, limit, items.len())?;

    // 逐条解析专辑信息
    let albums: Vec<Value> = items
        .iter()
        .map(|item| parse_album(item, separator))
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

    /// 验证歌手条目解析和分页计算逻辑。
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

        // 第 2 页，总数 45，已加载 40，还有更多
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
        // 第 3 页，已加载 45，取完
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
        // 本页无返回数据，has_more 为 false
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

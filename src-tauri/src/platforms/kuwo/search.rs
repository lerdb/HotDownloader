//! 酷我音乐歌曲搜索模块。
//!
//! 通过酷我移动端搜索接口 `http://search.kuwo.cn/r.s` 进行歌曲搜索。
//! 搜索结果中的每首歌曲通过 [`parse_song`] 解析为统一格式。
//!
//! 关键点：
//! - 酷我的 `SHOW` 字段是本次返回的歌曲数，`TOTAL` 是搜索词匹配总数。
//! - 由于酷我接口的分页参数 `pn` 是从 0 开始的偏移量，
//!   为兼容前端已有的 `page`（从 1 开始）语义，内部转换 `pn = page - 1`。
//! - 搜索时**不拉取封面**：`coverUrl` 字段留空，由前端通过单独接口
//!   `fetch_cover(songId)` 按需加载，避免搜索接口被大量并发封面请求拖慢。

use serde_json::{json, Value};
use tauri::AppHandle;
use urlencoding::encode;

use super::parser::parse_song;
use crate::utils::filename::get_artist_separator;
use crate::utils::http::CLIENT;

/// 搜索歌曲，返回 JSON 字符串，包含 `songs` 和 `has_more`。
///
/// 该命令调用酷我移动端搜索接口，根据关键字、页码和每页数量搜索歌曲。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/musicSearch.js#L18>
///
/// # 参数
/// - `keyword`: 搜索关键字。
/// - `page`: 页码（从 1 开始）。
/// - `limit`: 每页歌曲数量。
///
/// # 返回
/// - `Ok(String)`：JSON 字符串，包含 `songs`（歌曲数组，`coverUrl` 留空）和 `has_more`。
/// - `Err(String)`：错误信息。
pub(crate) async fn search_songs(
    app_handle: &AppHandle,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    // 酷我 pn 从 0 开始，page 从 1 开始
    let pn = page.saturating_sub(1);

    let url = format!(
        "http://search.kuwo.cn/r.s?client=kt&all={}&pn={}&rn={}\
         &uid=794762570&ver=kwplayer_ar_9.2.2.1&vipver=1&show_copyright_off=1\
         &newver=1&ft=music&cluster=0&strategy=2012&encoding=utf8\
         &rformat=json&vermerge=1&mobi=1&issubtitle=1",
        encode(&keyword),
        pn,
        limit
    );

    let resp = CLIENT
        .get(&url)
        .header("User-Agent", "kwplayer_ar_9.2.2.1")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 提取 abslist 数组
    let abslist = data["abslist"]
        .as_array()
        .ok_or("未找到歌曲列表（abslist 字段缺失）")?;

    // 解析每首歌曲（coverUrl 留空，由前端按需调用 fetch_cover 接口）
    let artist_separator = get_artist_separator(app_handle);
    let mut songs: Vec<Value> = Vec::new();
    for item in abslist {
        if let Some(song_obj) = parse_song(item, &artist_separator) {
            songs.push(song_obj);
        }
    }

    // 分页判断：酷我的 `TOTAL` 字段表示搜索结果总数，`pn*rn` 为当前 offset。
    // 当 offset + 本次返回数 < TOTAL 时，说明还有下一页。
    // 注意 `TOTAL` 是字符串字段，需要解析为整数。
    let total: u64 = data["TOTAL"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| data["TOTAL"].as_u64())
        .unwrap_or(0);
    let offset = (pn as u64) * (limit as u64);
    let returned = songs.len() as u64;
    let has_more = offset + returned < total;

    let result = json!({
        "songs": songs,
        "has_more": has_more
    });

    serde_json::to_string(&result).map_err(|e| format!("序列化结果失败: {}", e))
}

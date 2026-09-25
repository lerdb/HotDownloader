//! 歌单歌曲获取模块。
//!
//! 支持从歌单链接或歌单 ID 中提取歌单标识，并调用 QQ 音乐接口获取歌单信息和歌曲列表。
//! 歌曲列表中的每一项通过 [`super::parser::parse_song`] 解析为统一格式。

use reqwest::Url;
use serde_json::{json, Value};

use super::parser::parse_song;
use crate::platforms::CLIENT;

/// 从用户输入中提取歌单 ID。
///
/// 支持三种输入格式：
/// 1. 纯数字字符串：直接作为歌单 ID。
/// 2. URL 中包含 `id` 查询参数：例如 `https://y.qq.com/n/yqq/playsquare/123.html?id=456`，提取 `id` 的值。
/// 3. URL 路径中包含 `playlist` 段：例如 `https://y.qq.com/n/yqq/playlist/123.html`，提取 `123`。
///
/// # 参数
/// - `input`: 用户输入的字符串，可能是歌单链接或纯数字 ID。
///
/// # 返回
/// - `Ok(String)`：提取出的歌单 ID（纯数字字符串）。
/// - `Err(String)`：输入为空、无法解析为 URL、或无法从 URL 中提取出有效 ID。
fn extract_playlist_id(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("请输入歌单链接或 ID".into());
    }

    // 如果输入全部是数字，直接作为 ID
    if input.chars().all(|c| c.is_ascii_digit()) {
        return Ok(input.to_string());
    }

    // 否则尝试解析为 URL
    let url = Url::parse(input).map_err(|_| "无法识别的歌单链接或 ID".to_string())?;

    // 先检查查询参数中的 id
    if let Some((_, id)) = url.query_pairs().find(|(k, _)| k == "id") {
        let id = id.trim().to_string();
        if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
            return Ok(id);
        }
    }

    // 再检查路径中包含 "playlist" 的段，其后的段为 ID
    if let Some(segments) = url.path_segments() {
        let segs: Vec<&str> = segments.collect();
        if let Some(pos) = segs.iter().position(|s| *s == "playlist") {
            if let Some(id_part) = segs.get(pos + 1) {
                // 去掉可能的 ".html" 后缀
                let id = id_part.trim_end_matches(".html");
                if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                    return Ok(id.to_string());
                }
            }
        }
    }

    Err("无法从链接中提取歌单 ID".into())
}

/// 获取歌单歌曲列表。
///
/// 该命令接收用户输入的歌单链接或 ID，先提取歌单 ID，然后调用 QQ 音乐歌单接口获取歌单信息和歌曲列表。
/// 每首歌曲通过 [`parse_song`] 解析，过滤无法解析的条目。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/songList.js#L196>
///
/// # 参数
/// - `input`: 歌单链接或歌单 ID 字符串。
///
/// # 返回
/// - `Ok(String)`：JSON 字符串，包含 `playlist`（歌单信息）和 `songs`（歌曲列表数组）两个字段。
/// - `Err(String)`：错误信息，例如 ID 提取失败、接口错误、解析失败等。
pub async fn fetch_playlist_songs(separator: &str, input: String) -> Result<String, String> {
    // 从输入中提取歌单 ID
    let disstid = extract_playlist_id(&input)?;

    // 构建请求 URL
    let base_url = "https://c.y.qq.com/qzone/fcg-bin/fcg_ucc_getcdinfo_byids_cp.fcg";
    let url = Url::parse_with_params(
        base_url,
        &[
            ("type", "1"),
            ("json", "1"),
            ("utf8", "1"),
            ("onlysong", "0"),
            ("new_format", "1"),
            ("disstid", disstid.as_str()),
            ("loginUin", "0"),
            ("hostUin", "0"),
            ("format", "json"),
            ("inCharset", "utf8"),
            ("outCharset", "utf-8"),
            ("notice", "0"),
            ("platform", "yqq.json"),
            ("needNewCode", "0"),
        ],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))?;

    // 发送 GET 请求
    let resp = CLIENT
        .get(url)
        .header(
            "Referer",
            format!("https://y.qq.com/n/yqq/playsquare/{}.html", disstid),
        )
        .header("Origin", "https://y.qq.com")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Accept", "*/*")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 检查接口返回码
    let code = data["code"].as_i64().unwrap_or(-1);
    let subcode = data["subcode"].as_i64().unwrap_or(-1);
    if code != 0 || subcode != 0 {
        return Err(format!(
            "接口错误: code={}, subcode={}, msg={}",
            code,
            subcode,
            data["msg"].as_str().unwrap_or("")
        ));
    }

    // 提取歌单信息（cdlist 数组的第一项）
    let cd = data["cdlist"]
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or("未找到歌单数据")?;

    // 构建歌单基本信息
    let playlist = json!({
        "id": disstid,
        "name": cd["dissname"].as_str().unwrap_or(""),
        "creator": cd["nickname"].as_str().unwrap_or(""),
        "coverUrl": cd["logo"].as_str().unwrap_or(""),
        "songCount": cd["songnum"].as_u64().unwrap_or(0),
        "playCount": cd["visitnum"].as_u64().unwrap_or(0),
    });

    // 获取歌曲列表并逐首解析
    let songlist = cd["songlist"].as_array().ok_or("未找到歌曲列表")?;

    let mut songs: Vec<Value> = Vec::new();

    for song in songlist {
        if let Some(song_obj) = parse_song(song, separator) {
            songs.push(song_obj);
        }
    }

    Ok(json!({
        "playlist": playlist,
        "songs": songs
    })
    .to_string())
}

/// 搜索 QQ 音乐歌单。
///
/// 调用 QQ 音乐歌单搜索接口 `http://c.y.qq.com/soso/fcgi-bin/client_music_search_songlist`。
/// 参数 `page` 从 1 开始，内部转换为接口要求的 `page_no`。
/// 分页判断依据接口返回的 `sum`（总歌单数）和当前 offset 计算。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/songList.js#L292>
///
/// # 参数
/// - `keyword`: 搜索关键词。
/// - `page`: 页码。
/// - `limit`: 每页数量。
///
/// # 返回
/// - `Ok(String)`：JSON 字符串，包含 `playlists`（歌单数组）和 `has_more`。
/// - `Err(String)`：错误信息。
pub async fn search_playlists(keyword: String, page: u32, limit: u32) -> Result<String, String> {
    // 接口的 page_no 从 0 开始，前端 page 从 1 开始，需要减一
    let page_no = page.saturating_sub(1);

    // 构建请求 URL
    let url = Url::parse_with_params(
        "http://c.y.qq.com/soso/fcgi-bin/client_music_search_songlist",
        &[
            ("page_no", page_no.to_string().as_str()),
            ("num_per_page", limit.to_string().as_str()),
            ("format", "json"),
            ("query", keyword.as_str()),
            ("remoteplace", "txt.yqq.playlist"),
            ("inCharset", "utf8"),
            ("outCharset", "utf-8"),
        ],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))?;

    // 发送 GET 请求，必须携带 Referer 和 User-Agent
    let resp = CLIENT
        .get(url)
        .header("Referer", "http://y.qq.com/portal/search.html")
        .header(
            "User-Agent",
            "Mozilla/5.0 (compatible; MSIE 9.0; Windows NT 6.1; WOW64; Trident/5.0)",
        )
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 检查返回码
    let code = data["code"].as_i64().unwrap_or(-1);
    let subcode = data["subcode"].as_i64().unwrap_or(-1);
    if code != 0 || subcode != 0 {
        // 如果没有 data 字段，可能是 "ids empty 0" 这类提示
        if data.get("data").is_none() {
            // 无结果视为正常，返回空列表
            return Ok(json!({
                "playlists": [],
                "has_more": false
            })
            .to_string());
        }
        return Err(format!(
            "接口错误: code={}, subcode={}, message={}",
            code,
            subcode,
            data["message"].as_str().unwrap_or("")
        ));
    }

    // 提取歌单列表
    let list = data["data"]["list"]
        .as_array()
        .map(|arr| arr.to_vec())
        .unwrap_or_default();

    // 解析每个歌单
    let mut playlists: Vec<Value> = Vec::new();
    for item in list {
        let playlist_id = item["dissid"].as_str().unwrap_or("").to_string();
        if playlist_id.is_empty() {
            continue;
        }
        let name = item["dissname"].as_str().unwrap_or("").to_string();
        let creator = item["creator"]["name"].as_str().unwrap_or("").to_string();
        let cover_url = item["imgurl"].as_str().unwrap_or("").to_string();
        let song_count = item["song_count"].as_u64().unwrap_or(0);
        let play_count = item["listennum"].as_u64().unwrap_or(0);
        let introduction = item["introduction"].as_str().unwrap_or("").to_string();

        playlists.push(json!({
            "id": playlist_id,
            "name": name,
            "creator": creator,
            "coverUrl": cover_url,
            "songCount": song_count,
            "playCount": play_count,
            "introduction": introduction
        }));
    }

    // 分页判断：page_no 是当前页码（从 0 开始），sum 是总歌单数
    let sum: u64 = data["data"]["sum"].as_u64().unwrap_or(0);
    let offset = (page_no as u64) * (limit as u64);
    let returned = playlists.len() as u64;
    let has_more = offset + returned < sum;

    Ok(json!({
        "playlists": playlists,
        "has_more": has_more
    })
    .to_string())
}

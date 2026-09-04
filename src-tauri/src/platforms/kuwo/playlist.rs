//! 酷我音乐歌单获取模块。
//!
//! 支持从歌单链接或歌单 ID 中提取酷我歌单标识，调用酷我歌单接口获取歌单信息和歌曲列表。
//! 歌曲列表中的每一项通过字段适配后由 [`super::parser::parse_song`] 解析为统一格式。
//!
//! 关键点：
//! - 酷我歌单接口返回的歌曲字段与搜索接口不同（如 `name` 对应 `SONGNAME`，`artist` 对应 `ARTIST`，`album` 对应 `ALBUM`，`duration` 为字符串），
//!   且没有 `MUSICRID` 字段，只有纯数字 `id` 字段。
//! - 接口分页参数 `pn` 从 0 开始，`rn` 为每页数量，本项目固定请求 `rn=1000` 以尽量一次拿完；
//!   若歌单 `total` 超过 1000，自动分页拉取剩余歌曲。

use serde_json::{json, Map, Value};
use tauri::AppHandle;
use url::Url;

use super::parser::parse_song;
use crate::utils::filename::get_artist_separator;
use crate::utils::http::CLIENT;

/// 从用户输入中提取酷我歌单 ID。
///
/// 支持输入格式：
/// 1. 纯数字字符串：直接作为歌单 ID。
/// 2. URL 中包含 `playlist_detail/数字` 或 `playlist/数字` 的路径：
///    - `https://kuwo.cn/playlist_detail/3689581612`
///    - `https://m.kuwo.cn/h5app/playlist/2736267853?t=qqfriend`
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/songList.js#L27>
///
/// # 参数
/// - `input`: 用户输入的字符串，可能是歌单链接或纯数字 ID。
///
/// # 返回
/// - `Ok(String)`：提取出的歌单 ID（纯数字字符串）。
/// - `Err(String)`：输入为空、无法解析或提取失败。
fn extract_playlist_id(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("请输入歌单链接或 ID".into());
    }

    // 如果输入全部是数字，直接作为 ID
    if input.chars().all(|c| c.is_ascii_digit()) {
        return Ok(input.to_string());
    }

    // 解析为 URL
    let url = Url::parse(input).map_err(|_| "无法识别的歌单链接或 ID".to_string())?;

    // 提取路径段，查找 "playlist_detail" 或 "playlist" 后紧跟的数字段
    if let Some(segments) = url.path_segments() {
        let segs: Vec<&str> = segments.collect();
        for (i, seg) in segs.iter().enumerate() {
            if *seg == "playlist_detail" || *seg == "playlist" {
                if let Some(id_part) = segs.get(i + 1) {
                    // 去掉可能的 ".html" 后缀
                    let id = id_part.trim_end_matches(".html");
                    if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                        return Ok(id.to_string());
                    }
                }
            }
        }
    }

    Err("无法从链接中提取歌单 ID".into())
}

/// 获取歌单歌曲列表。
///
/// 调用酷我歌单接口获取歌单信息和歌曲列表，自动处理分页（当歌曲总数超过 1000 时）。
/// 每首歌曲通过字段适配后由 [`parse_song`] 解析，过滤无法解析的条目。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/songList.js#L168>
///
/// # 参数
/// - `app_handle`: Tauri 应用句柄，用于读取歌曲信息分隔符设置。
/// - `input`: 歌单链接或歌单 ID 字符串。
///
/// # 返回
/// - `Ok(String)`：JSON 字符串，包含 `playlist`（歌单信息）和 `songs`（歌曲列表数组）两个字段。
/// - `Err(String)`：错误信息。
pub(crate) async fn fetch_playlist_songs(
    app_handle: &AppHandle,
    input: String,
) -> Result<String, String> {
    // 提取歌单 ID
    let pid = extract_playlist_id(&input)?;

    // 每页请求数量固定为 1000，尽量减少请求次数
    let rn: u32 = 1000;
    let mut all_musiclist: Vec<Value> = Vec::new();
    let mut playlist_info: Option<Value> = None;
    let mut pn: u32 = 0;

    loop {
        // 构建请求 URL
        let url = format!(
            "http://nplserver.kuwo.cn/pl.svc?op=getlistinfo&pid={}&pn={}&rn={}&encode=utf8&keyset=pl2012&identity=kuwo&pcmp4=1&vipver=MUSIC_9.0.5.0_W1&newver=1",
            pid, pn, rn
        );

        // 发送 GET 请求
        let resp = CLIENT
            .get(&url)
            .header("Referer", "http://www.kuwo.cn/")
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| format!("网络错误: {}", e))?;

        let text = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {}", e))?;
        let data: Value =
            serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

        // 检查接口返回结果
        let result = data["result"].as_str().unwrap_or("");
        if result != "ok" {
            return Err(format!("酷我歌单接口错误: result={}", result));
        }

        // 首次请求时提取歌单基本信息（后续分页请求不需要重复提取）
        if playlist_info.is_none() {
            let playlist = json!({
                "id": pid,
                "name": data["title"].as_str().unwrap_or(""),
                "creator": data["uname"].as_str().unwrap_or(""),
                "coverUrl": data["pic"].as_str().unwrap_or(""),
                "songCount": data["total"].as_u64().unwrap_or(0),
                "playCount": data["playnum"].as_u64().unwrap_or(0),
            });
            playlist_info = Some(playlist);
        }

        // 提取本页歌曲列表
        let musiclist = data["musiclist"].as_array().ok_or("未找到歌曲列表")?;
        all_musiclist.extend(musiclist.iter().cloned());

        // 判断是否还有下一页
        let total: u64 = data["total"].as_u64().unwrap_or(0);
        let returned_count = all_musiclist.len() as u64;
        if returned_count >= total {
            break;
        }

        // 准备下一页
        pn += 1;
        // 安全保护：避免无限循环（理论上 total 是有效的）
        if pn > 100 {
            break;
        }
    }

    // 适配字段并解析歌曲
    let artist_separator = get_artist_separator(app_handle);
    let mut songs: Vec<Value> = Vec::new();

    for item in all_musiclist {
        // 酷我歌单接口字段与搜索接口不同，需要适配
        let mut adapted = Map::new();
        // 保留原始 id 字段，parser 会优先解析
        if let Some(id_val) = item.get("id") {
            adapted.insert("id".to_string(), id_val.clone());
        }
        adapted.insert("SONGNAME".to_string(), item["name"].clone());
        adapted.insert("ARTIST".to_string(), item["artist"].clone());
        adapted.insert("ALBUM".to_string(), item["album"].clone());
        // 处理 duration：原始为字符串（秒），转为数字
        let duration = item["duration"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        adapted.insert("DURATION".to_string(), Value::from(duration));
        // 保留音质信息字段
        adapted.insert("N_MINFO".to_string(), item["N_MINFO"].clone());
        adapted.insert("MINFO".to_string(), item["MINFO"].clone());
        // 复制 albumpic 字段作为封面 URL（若存在），避免后续单独获取封面
        if let Some(albumpic) = item.get("albumpic") {
            adapted.insert("albumpic".to_string(), albumpic.clone());
        }

        let adapted_value = Value::Object(adapted);
        if let Some(song_obj) = parse_song(&adapted_value, &artist_separator) {
            songs.push(song_obj);
        }
    }

    // 构建最终返回 JSON
    let playlist = playlist_info.unwrap_or(json!({
        "id": pid,
        "name": "",
        "creator": "",
        "coverUrl": "",
        "songCount": 0,
        "playCount": 0,
    }));

    Ok(json!({
        "playlist": playlist,
        "songs": songs
    })
    .to_string())
}

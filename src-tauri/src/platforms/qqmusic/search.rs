//! 歌曲搜索模块。
//!
//! 通过 QQ 音乐移动端搜索接口进行歌曲搜索，返回解析后的歌曲列表和分页信息。
//! 歌曲解析复用 [`super::parser::parse_song`] 函数。

use serde_json::{json, Value};
use tauri::AppHandle;

use super::parser::parse_song;
use crate::utils::filename::get_artist_separator;
use crate::utils::guid::get_guid;
use crate::utils::http::CLIENT;

/// 搜索歌曲，返回 JSON 数组字符串（扩展 SongInfo，增加 `mediaMid` 和 `qualities`）。
///
/// 该命令调用 QQ 音乐移动端搜索接口，根据关键字、页码和每页数量搜索歌曲。
/// 请求中需要动态生成 `searchid` 和 `guid`，并携带设备信息等参数以模拟真实客户端。
/// 搜索结果中的每首歌曲通过 [`parse_song`] 解析，并返回分页标志 `has_more`。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/musicSearch.js#L13>
///
/// # 参数
/// - `keyword`: 搜索关键字。
/// - `page`: 页码（从 1 开始）。
/// - `limit`: 每页歌曲数量。
///
/// # 返回
/// - `Ok(String)`：JSON 字符串，包含 `songs`（歌曲数组）和 `has_more`（是否有下一页）两个字段。
/// - `Err(String)`：错误信息，包括网络错误、接口错误、序列化错误等。
pub(crate) async fn search_songs(
    app_handle: &AppHandle,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    // 生成搜索 ID（当前毫秒时间戳）
    let searchid = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();

    // 构造请求体
    let request_body = json!({
        "comm": {
            "ct": "11",
            "cv": "14090508",
            "v": "14090508",
            "tmeAppID": "qqmusic",
            "guid": get_guid(),
            "phonetype": "EBG-AN10",
            "deviceScore": "553.47",
            "devicelevel": "50",
            "newdevicelevel": "20",
            "rom": "HuaWei/EMOTION/EmotionUI_14.2.0",
            "os_ver": "12",
            "OpenUDID": "0",
            "OpenUDID2": "0",
            "QIMEI36": "0",
            "udid": "0",
            "chid": "0",
            "aid": "0",
            "oaid": "0",
            "taid": "0",
            "tid": "0",
            "wid": "0",
            "uid": "0",
            "sid": "0",
            "modeSwitch": "6",
            "teenMode": "0",
            "ui_mode": "2",
            "nettype": "1020",
            "v4ip": ""
        },
        "req": {
            "module": "music.search.SearchCgiService",
            "method": "DoSearchForQQMusicMobile",
            "param": {
                "search_type": 0,
                "searchid": searchid,
                "query": keyword,
                "page_num": page,
                "num_per_page": limit,   // 使用参数控制每页数量
                "highlight": 0,
                "nqc_flag": 0,
                "multi_zhida": 0,
                "cat": 2,
                "grp": 1,
                "sin": 0,
                "sem": 0
            }
        }
    });

    // 发送 POST 请求
    let resp = CLIENT
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 检查整体状态
    if data["code"] != 0 {
        return Err(format!("接口错误: code={}", data["code"]));
    }
    let req = &data["req"];
    if req["code"] != 0 {
        return Err(format!("搜索错误: req.code={}", req["code"]));
    }

    // 提取歌曲列表
    let item_song = req["data"]["body"]["item_song"]
        .as_array()
        .ok_or("未找到歌曲列表")?;

    // 分页判断修改
    // 避免因 parse_song 过滤导致有效歌曲数不足一页时，误判为无更多结果，影响“加载更多”按钮显示。
    // 直接读取接口返回的 meta.nextpage 字段。该字段为 -1 表示无下一页，否则为下一页页码。
    let meta = &req["data"]["meta"];
    let nextpage = meta["nextpage"].as_i64().unwrap_or(-1);
    let has_more = nextpage != -1;

    // 逐首解析歌曲，过滤无法解析的条目
    let artist_separator = get_artist_separator(app_handle);
    let mut songs: Vec<Value> = Vec::new();
    for item in item_song {
        if let Some(song_obj) = parse_song(item, &artist_separator) {
            songs.push(song_obj);
        }
    }

    // 返回包含歌曲列表和分页标志的 JSON 对象
    let result = json!({
        "songs": songs,
        "has_more": has_more
    });

    serde_json::to_string(&result).map_err(|e| format!("序列化结果失败: {}", e))
}

//! 热搜关键词与搜索建议模块。
//!
//! 提供热搜关键词获取和搜索自动补全建议两个 Tauri 命令。
//! 热搜数据来自腾讯音乐热搜接口，建议数据来自 QQ 音乐智能搜索接口。

use serde_json::{json, Value};
use url::Url;

use crate::utils::guid::get_guid;
use crate::utils::http::CLIENT;

/// 获取热搜关键词列表，返回 JSON 数组字符串。
///
/// 调用腾讯音乐热搜接口，获取当前热门搜索关键词，最多返回前 30 个非空关键词。
/// 请求需要 `guid` 和设备信息等公共参数。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/hotSearch.js#L15>
///
/// # 返回
/// - `Ok(String)`：JSON 数组字符串，每个元素为热搜关键词字符串。
/// - `Err(String)`：错误信息，包括网络错误、接口错误、数据缺失等。
pub(crate) async fn fetch_hot_keywords() -> Result<String, String> {
    // 构造请求体，模拟 PC 端请求
    let request_body = json!({
        "comm": {
            "ct": "19",
            "cv": "1803",
            "guid": get_guid(),
            "patch": "118",
            "psrf_access_token_expiresAt": 0,
            "psrf_qqaccess_token": "",
            "psrf_qqopenid": "",
            "psrf_qqunionid": "",
            "tmeAppID": "qqmusic",
            "tmeLoginType": 0,
            "uin": "0",
            "wid": "0"
        },
        "hotkey": {
            "module": "tencent_musicsoso_hotkey.HotkeyService",
            "method": "GetHotkeyForQQMusicPC",
            "param": {
                "search_id": "",
                "uin": 0
            }
        }
    });

    // 发送 POST 请求
    let resp = CLIENT
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Content-Type", "application/json")
        .header("Referer", "https://y.qq.com/portal/player.html")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 热搜数据在独立的 "hotkey" 字段中
    let hotkey = &data["hotkey"];
    if hotkey.is_null() {
        return Err("热搜数据缺失".into());
    }
    let code = hotkey["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(format!("热搜接口错误: code={}", code));
    }

    // 提取热搜关键词数组
    let vec_hotkey = hotkey["data"]["vec_hotkey"]
        .as_array()
        .ok_or("未找到热搜列表")?;

    // 收集前 30 个非空关键词
    let mut keywords = Vec::new();
    for item in vec_hotkey.iter().take(30) {
        if let Some(q) = item["query"].as_str() {
            if !q.is_empty() {
                keywords.push(q.to_string());
            }
        }
    }

    serde_json::to_string(&keywords).map_err(|e| format!("序列化结果失败: {}", e))
}

/// 获取搜索建议。
///
/// 根据用户输入的关键字，调用 QQ 音乐智能搜索接口，返回按类型（歌曲、歌手、专辑、MV）分组的搜索建议列表。
/// 建议数据中的每个条目包含 `id`、`mid`、`name`、`singer`、`cover` 等字段，
/// 其中 MV 类型额外包含 `vid` 字段。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/tipSearch.js#L10>
///
/// # 参数
/// - `keyword`: 用户输入的关键字。
///
/// # 返回
/// - `Ok(String)`：JSON 对象字符串，包含 `song`、`singer`、`album`、`mv` 四个键，
///   每个键对应一个建议条目数组。
/// - `Err(String)`：错误信息，包括网络错误、接口错误、数据缺失等。
pub(crate) async fn fetch_suggestions(keyword: String) -> Result<String, String> {
    // 构建 URL，并进行 URL 编码
    let base_url = "https://c.y.qq.com/splcloud/fcgi-bin/smartbox_new.fcg";
    let url = Url::parse_with_params(
        base_url,
        &[
            ("is_xml", "0"),
            ("format", "json"),
            ("key", &keyword),
            ("loginUin", "0"),
            ("hostUin", "0"),
            ("inCharset", "utf8"),
            ("outCharset", "utf-8"),
            ("notice", "0"),
            ("platform", "yqq"),
            ("needNewCode", "0"),
        ],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))?;

    // 发送 GET 请求
    let resp = CLIENT
        .get(url)
        .header("Referer", "https://y.qq.com/portal/player.html")
        .header("Accept", "*/*")
        .header("Host", "c.y.qq.com")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 检查状态码
    let code = data["code"].as_i64().unwrap_or(-1);
    let subcode = data["subcode"].as_i64().unwrap_or(-1);
    if code != 0 || subcode != 0 {
        return Err(format!("接口错误: code={}, subcode={}", code, subcode));
    }

    let root_data = data["data"].as_object().ok_or("缺少 data 字段")?;

    // 定义需要提取的类型列表及其对应的字段名
    let types = vec![
        ("song", "单曲"),
        ("singer", "歌手"),
        ("album", "专辑"),
        ("mv", "MV"),
    ];

    let mut result = serde_json::Map::new();

    // 遍历每种类型，提取对应的建议条目
    for (type_key, _type_name) in types {
        let mut items = Vec::new();

        // 获取该类型下的 itemlist 数组
        if let Some(obj) = root_data.get(type_key).and_then(|v| v.as_object()) {
            if let Some(itemlist) = obj.get("itemlist").and_then(|v| v.as_array()) {
                for item in itemlist {
                    let mut map = serde_json::Map::new();
                    // 通用字段
                    if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                        map.insert("id".to_string(), json!(id));
                    }
                    if let Some(mid) = item.get("mid").and_then(|v| v.as_str()) {
                        map.insert("mid".to_string(), json!(mid));
                    }
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        map.insert("name".to_string(), json!(name));
                    }
                    if let Some(singer) = item.get("singer").and_then(|v| v.as_str()) {
                        map.insert("singer".to_string(), json!(singer));
                    }
                    // 封面图片（歌手、专辑可能有，单曲通常没有）
                    if let Some(pic) = item.get("pic").and_then(|v| v.as_str()) {
                        map.insert("cover".to_string(), json!(pic));
                    } else {
                        map.insert("cover".to_string(), json!(null));
                    }
                    // MV 特有字段 vid
                    if type_key == "mv" {
                        if let Some(vid) = item.get("vid").and_then(|v| v.as_str()) {
                            map.insert("vid".to_string(), json!(vid));
                        }
                    }
                    items.push(Value::Object(map));
                }
            }
        }
        result.insert(type_key.to_string(), json!(items));
    }

    serde_json::to_string(&Value::Object(result)).map_err(|e| format!("序列化结果失败: {}", e))
}

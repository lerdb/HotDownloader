//! 酷我音乐热搜关键词与搜索建议模块。
//!
//! 提供热搜关键词获取和搜索自动补全建议两个功能。
//! 热搜数据来自 `hotword.kuwo.cn`，建议数据来自 `tips.kuwo.cn`。

use serde_json::{json, Value};
use url::Url;

use crate::utils::http::CLIENT;

/// 获取热搜关键词列表，返回 JSON 数组字符串。
///
/// 调用 `http://hotword.kuwo.cn/hotword.s` 获取当前热门搜索关键词。
/// 返回数组中 `tagvalue` 下的每项包含 `key`（关键词）、`popularity` 等字段。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/hotSearch.js#L9>
///
/// # 返回
/// - `Ok(String)`：JSON 数组字符串，每个元素为热搜关键词字符串。
/// - `Err(String)`：错误信息，包括网络错误、接口错误、数据缺失等。
pub(crate) async fn fetch_hot_keywords() -> Result<String, String> {
    let url = Url::parse_with_params(
        "http://hotword.kuwo.cn/hotword.s",
        &[
            ("prod", "kwplayer_ar_9.3.0.1"),
            ("corp", "kuwo"),
            ("newver", "2"),
            ("vipver", "9.3.0.1"),
            ("source", "kwplayer_ar_9.3.0.1_40.apk"),
            ("p2p", "1"),
            ("notrace", "0"),
            ("uid", "0"),
            ("plat", "kwplayer_ar"),
            ("rformat", "json"),
            ("encoding", "utf8"),
            ("tabid", "1"),
        ],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))?;

    let resp = CLIENT
        .get(url)
        .header("User-Agent", "okhttp/4.10.0")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 提取 tagvalue 数组，每项包含 key 字段
    let arr = data["tagvalue"]
        .as_array()
        .ok_or("未找到热搜列表（tagvalue 字段缺失）")?;

    // 收集前 30 个非空关键词
    let mut keywords: Vec<String> = Vec::new();
    for item in arr.iter().take(30) {
        if let Some(k) = item["key"].as_str() {
            if !k.is_empty() {
                keywords.push(k.to_string());
            }
        }
    }

    serde_json::to_string(&keywords).map_err(|e| format!("序列化结果失败: {}", e))
}

/// 获取搜索建议。
///
/// 根据用户输入的关键字，调用酷我 `tips.kuwo.cn` 智能搜索接口，返回建议条目数组。
/// 由于酷我建议接口只返回混合类型的条目（无法区分子类型），
/// 此处把整个 `WORDITEMS` 列表放入 `song` 数组返回，保持与前端类型一致。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/tipSearch.js#L15>
///
/// # 参数
/// - `keyword`: 用户输入的关键字。
///
/// # 返回
/// - `Ok(String)`：JSON 对象字符串，包含 `song`（建议条目数组）、`singer`、`album`、`mv` 四个键，
///   其余三键均为空数组。
/// - `Err(String)`：错误信息。
pub(crate) async fn fetch_suggestions(keyword: String) -> Result<String, String> {
    let url = Url::parse_with_params(
        "https://tips.kuwo.cn/t.s",
        &[
            ("corp", "kuwo"),
            ("newver", "3"),
            ("p2p", "1"),
            ("notrace", "0"),
            ("c", "mbox"),
            ("w", &keyword),
            ("encoding", "utf8"),
            ("rformat", "json"),
        ],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))?;

    let resp = CLIENT
        .get(url)
        .header("Referer", "http://www.kuwo.cn/")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 检查 STATUS 字段
    let status = data["STATUS"].as_str().unwrap_or("");
    if status != "OK" {
        return Err(format!("建议接口错误: STATUS={}", status));
    }

    let items = data["WORDITEMS"].as_array();
    let mut song_items: Vec<Value> = Vec::new();

    if let Some(arr) = items {
        for item in arr {
            // 通用字段提取
            let name = item["RELWORD"].as_str().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            // 酷我建议项没有标准的 id/mid，使用 RELWORD 作为 name
            // RESOURCE_TYPE 可作为 id 备用，TAG_TYPE/TAG_IMG 等保留扩展字段
            let mut obj = json!({
                "id": name,
                "mid": name,
                "name": name,
                "singer": null,
                "cover": null,
            });
            // 保留原始数据备用（前端类型兼容未知字段）
            if let Some(t) = item.get("TAG_TYPE") {
                obj["tag_type"] = t.clone();
            }
            if let Some(img) = item.get("TAG_IMG").and_then(|v| v.as_str()) {
                obj["tag_img"] = json!(img);
            }
            song_items.push(obj);
        }
    }

    // 按前端 SearchSuggestionData 结构返回，其余分组留空
    let result = json!({
        "song": song_items,
        "singer": [],
        "album": [],
        "mv": [],
    });
    serde_json::to_string(&result).map_err(|e| format!("序列化结果失败: {}", e))
}

use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::path::Path;
use tauri::command;
use url::Url;

/// 全局复用 HTTP 客户端，启用连接池、超时等
pub(crate) static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("HotDownloader/1.0")
        .timeout(std::time::Duration::from_secs(30)) // 整体请求超时
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
});

/// 搜索歌曲，返回 JSON 数组字符串（扩展 SongInfo，增加 mediaMid 和 qualities）
/// https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/musicSearch.js#L13
#[command]
pub async fn search_songs(keyword: String, page: u32, limit: u32) -> Result<String, String> {
    let searchid = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();

    let request_body = json!({
        "comm": {
            "ct": "11",
            "cv": "14090508",
            "v": "14090508",
            "tmeAppID": "qqmusic",
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

    let mut songs = Vec::new();
    for item in item_song {
        if let Some(song_obj) = parse_song(item) {
            songs.push(song_obj);
        }
    }

    // 返回包含歌曲列表和分页标志的 JSON 对象
    let result = json!({
        "songs": songs,
        "has_more": has_more
    });

    Ok(serde_json::to_string(&result).map_err(|e| format!("序列化结果失败: {}", e))?)
}

/// 通用歌曲解析函数
/// - song: 歌曲原始 JSON 对象（搜索或歌单接口中的一项）
/// 标题优先使用 `name` 字段，若为空则使用 `title` 字段
/// 封面优先使用专辑 mid，其次歌手 mid，否则为空字符串
/// 返回 Option<Value>，当 mid 或 media_mid 为空时返回 None
fn parse_song(song: &Value) -> Option<Value> {
    // 歌曲唯一标识（使用 mid）
    let mid = song["mid"].as_str().unwrap_or("").to_string();
    if mid.is_empty() {
        return None;
    }

    // 数字歌曲 ID，用于歌词接口等需要数字 ID 的场景。
    // 兼容字段可能以数字或字符串形式出现。
    let song_id = song["id"]
        .as_u64()
        .or_else(|| song["id"].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0);

    // 必须有 media_mid 才能下载，否则跳过
    let media_mid = song["file"]["media_mid"].as_str().unwrap_or("").to_string();
    if media_mid.is_empty() {
        return None;
    }

    // 标题：优先 name，若为空则 title
    let title = song["name"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            song["title"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();

    // 歌手列表，用逗号连接
    let singers: Vec<String> = song["singer"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let artist = singers.join(", ");

    // 专辑名
    let album_name = song["album"]["name"].as_str().unwrap_or("").to_string();

    // 封面：专辑 mid 优先，其次歌手 mid
    let album_mid = song["album"]["mid"].as_str().unwrap_or("");
    let first_singer_mid = song["singer"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|s| s["mid"].as_str())
        .unwrap_or("");

    // 封面URL，同样用可变变量避免 if 表达式问题
    let mut cover_url = String::new();
    if !album_mid.is_empty() && album_mid != "空" {
        cover_url = format!(
            "https://y.gtimg.cn/music/photo_new/T002R500x500M000{}.jpg",
            album_mid
        );
    } else if !first_singer_mid.is_empty() {
        cover_url = format!(
            "https://y.gtimg.cn/music/photo_new/T001R500x500M000{}.jpg",
            first_singer_mid
        );
    }

    // 品质列表（复用 build_qualities）
    let qualities = build_qualities(&song["file"], &song["vs"]);

    Some(json!({
        "id": song_id,
        "mid": mid,
        "title": title,
        "artist": artist,
        "album": album_name,
        "coverUrl": cover_url,
        "mediaMid": media_mid,
        "qualities": qualities
    }))
}

/// 根据 file 和 vs 生成可用品质列表
fn build_qualities(file: &Value, vs: &Value) -> Vec<Value> {
    let media_mid = file["media_mid"].as_str().unwrap_or("");
    let mut list = Vec::new();

    // 标准品质，按顺序定义 (前端标签, 文件前缀, 后缀, 文件大小字段名)
    let standard_qualities: Vec<(&str, &str, &str, &str)> = vec![
        ("48kacc", "C200", ".m4a", "size_48aac"),
        ("96kacc", "C400", ".m4a", "size_96aac"),
        ("192kacc", "C600", ".m4a", "size_192aac"),
        ("96kogg", "O4M0", ".mgg", "size_96ogg"),
        ("192kogg", "O6M0", ".mgg", "size_192ogg"),
        ("128kmp3", "M500", ".mp3", "size_128mp3"),
        ("320kmp3", "M800", ".mp3", "size_320mp3"),
        ("ape", "A000", ".ape", "size_ape"),
        ("flac", "F0M0", ".mflac", "size_flac"),
        ("hires", "RSM1", ".mflac", "size_hires"),
    ];

    for (label, prefix, suffix, size_key) in &standard_qualities {
        let size = file[*size_key].as_u64().unwrap_or(0);
        if size > 0 {
            list.push(json!({
                "quality": label,
                "filename": format!("{}{}{}", prefix, media_mid, suffix),
                "size": size
            }));
        }
    }

    // 特殊品质：杜比全景声 → 臻品全景声 → 臻品母带（按此顺序）
    let size_new = file["size_new"].as_array();
    let vs_arr = vs.as_array();
    if let (Some(size_new), Some(vs_arr)) = (size_new, vs_arr) {
        let vs3 = vs_arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
        let vs4 = vs_arr.get(4).and_then(|v| v.as_str()).unwrap_or("");

        // 杜比全景声 (size_new[1] + vs[4])
        let size_dolby = size_new.get(1).and_then(|v| v.as_u64()).unwrap_or(0);
        if size_dolby > 0 && !vs4.is_empty() {
            list.push(json!({
                "quality": "杜比全景声",
                "filename": format!("Q0M0{}.mflac", vs4),
                "size": size_dolby
            }));
        }

        // 臻品全景声 (size_new[2] + vs[4])
        let size_panorama = size_new.get(2).and_then(|v| v.as_u64()).unwrap_or(0);
        if size_panorama > 0 && !vs4.is_empty() {
            list.push(json!({
                "quality": "臻品全景声",
                "filename": format!("Q0M1{}.mflac", vs4),
                "size": size_panorama
            }));
        }

        // 臻品母带 (size_new[0] + vs[3])
        let size_master = size_new.get(0).and_then(|v| v.as_u64()).unwrap_or(0);
        if size_master > 0 && !vs3.is_empty() {
            list.push(json!({
                "quality": "臻品母带",
                "filename": format!("AIM0{}.mflac", vs3),
                "size": size_master
            }));
        }
    }

    list
}

fn plain_link_error(result_code: i64, tips: &str) -> String {
    match result_code {
        104003 => "无法获取该音质的下载链接（可能需要登录，或该歌曲暂无此音质）".to_string(),
        104004 => "该歌曲已下架或禁止下载".to_string(),
        0 => "无法获取该音质的下载链接".to_string(),
        _ => {
            let tips = tips.trim();
            if tips.is_empty() {
                format!("获取下载链接失败，错误码: {}", result_code)
            } else {
                format!("获取下载链接失败，错误码: {}，{}", result_code, tips)
            }
        }
    }
}

/// 加密文件（.mgg / .mflac）专用，同时获取 purl 和 ekey
/// https://github.com/chrisdong/FileHub/blob/e1d752e1f29f877b7c895ae5aaff32a179fad051/root/importURLs/lxmusic/HeiMusic%E8%81%9A%E5%90%88%E6%BA%90_v1.1.5.js#L287
async fn fetch_encrypted_link(song_mid: &str, filename: &str) -> Result<(String, String), String> {
    let request_body = json!({
        "comm": {
            "ct": "19",
            "cv": "0",
            "guid": "",
            "tmeAppID": "qqmusic",
            "qq": "0"
        },
        "music.vkey.GetEVkey.CgiGetHotVkey": {
            "module": "music.vkey.GetEVkey",
            "method": "CgiGetHotVkey",
            "param": {
                "filename": [filename],
                "songmid": [song_mid]
            }
        },
        "music.vkey.GetEVkey.GetEkey": {
            "module": "music.vkey.GetEVkey",
            "method": "GetEkey",
            "param": {
                "finfo": [
                    {
                        "filename": filename,
                        "mid": song_mid
                    }
                ]
            }
        }
    });

    let resp = CLIENT
        .post("https://ut.y.qq.com/cgi-bin/musicu.fcg")
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

    // 提取 purl
    let vkey_resp = &data["music.vkey.GetEVkey.CgiGetHotVkey"];
    if vkey_resp["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("CgiGetHotVkey 错误: code={}", vkey_resp["code"]));
    }
    let urls = vkey_resp["data"]["urls"].as_array().ok_or("缺少 urls")?;
    let item = urls.get(0).ok_or("未获取到文件信息")?;
    let purl = item["purl"].as_str().unwrap_or("");
    // 检查是否有错误标记
    let result_code = item["result"].as_i64().unwrap_or(0);
    if purl.is_empty() || result_code != 0 {
        return Err(plain_link_error(result_code, ""));
    }

    // 提取 ekey
    let ekey_resp = &data["music.vkey.GetEVkey.GetEkey"];
    if ekey_resp["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("GetEkey 错误: code={}", ekey_resp["code"]));
    }
    let ekeyinfo = ekey_resp["data"]["ekeyinfo"]
        .as_array()
        .ok_or("缺少 ekeyinfo")?;
    let ekey = ekeyinfo
        .get(0)
        .and_then(|e| e["ekey"].as_str())
        .unwrap_or("")
        .to_string();

    // 拼接完整下载 URL（使用主 CDN）
    let full_url = format!("https://wx.music.tc.qq.com/{}", purl);
    Ok((full_url, ekey))
}

/// 非加密文件专用，仅获取 purl，无需密钥
/// https://github.com/lyswhut/lx-music-source/blob/55eb9881dad6ca895505352f3a0a7d1dfa3444e0/src/apis/tx.js#L30
async fn fetch_plain_link(song_mid: &str, filename: &str) -> Result<(String, String), String> {
    let request_body = json!({
        "comm": {
            "ct": 24,
            "cv": 0,
            "tmeAppID": "qqmusic",
            "format": "json"
        },
        "req_0": {
            "module": "vkey.GetVkeyServer",
            "method": "CgiGetVkey",
            "param": {
                "guid": "10000",
                "filename": [filename],
                "songmid": [song_mid],
                "songtype": [0]
            }
        }
    });

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

    // 检查外层 code
    if data["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("接口错误: code={}", data["code"]));
    }
    let req_0 = &data["req_0"];
    if req_0["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("请求错误: code={}", req_0["code"]));
    }

    let midurlinfo = req_0["data"]["midurlinfo"]
        .as_array()
        .ok_or("缺少 midurlinfo")?;
    let item = midurlinfo.get(0).ok_or("未找到歌曲信息")?;

    let purl = item["purl"].as_str().unwrap_or("");
    let result_code = item["result"].as_i64().unwrap_or(0);

    // 检查 purl 是否为空或 result 是否非0
    if purl.is_empty() || result_code != 0 {
        return Err(plain_link_error(
            result_code,
            item["tips"].as_str().unwrap_or(""),
        ));
    }

    let full_url = format!("https://wx.music.tc.qq.com/{}", purl);
    // 非加密文件无需密钥，返回空字符串
    Ok((full_url, String::new()))
}

/// 获取下载链接与解密密钥
/// 参数：song_id 为歌曲 mid，filename 为品质文件名（如 M800001abc.mp3）
/// 返回 (完整下载链接, 解密密钥)，非加密文件密钥为空
/// 核心函数：获取下载链接和密钥，供下载模块调用
/// 获取下载链接与解密密钥（对外统一入口）
pub(crate) async fn get_download_link(
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext == "mgg" || ext == "mflac" {
        fetch_encrypted_link(song_mid, filename).await
    } else {
        fetch_plain_link(song_mid, filename).await
    }
}

#[command]
pub async fn fetch_download_link(song_mid: String, filename: String) -> Result<String, String> {
    let (url, key) = get_download_link(&song_mid, &filename).await?;
    let result = json!({ "url": url, "key": key });
    Ok(result.to_string())
}

/// 获取热搜关键词列表，返回 JSON 数组字符串
/// https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/hotSearch.js#L15
#[command]
pub async fn fetch_hot_keywords() -> Result<String, String> {
    let request_body = json!({
        "comm": {
            "ct": "19",
            "cv": "1803",
            "guid": "0",
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

    let vec_hotkey = hotkey["data"]["vec_hotkey"]
        .as_array()
        .ok_or("未找到热搜列表")?;

    let mut keywords = Vec::new();
    for item in vec_hotkey.iter().take(30) {
        if let Some(q) = item["query"].as_str() {
            if !q.is_empty() {
                keywords.push(q.to_string());
            }
        }
    }

    Ok(serde_json::to_string(&keywords).map_err(|e| format!("序列化结果失败: {}", e))?)
}

/// 获取搜索建议
/// https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/tipSearch.js#L10
#[command]
pub async fn fetch_suggestions(keyword: String) -> Result<String, String> {
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

    for (type_key, _type_name) in types {
        let mut items = Vec::new();

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

    Ok(serde_json::to_string(&Value::Object(result))
        .map_err(|e| format!("序列化结果失败: {}", e))?)
}

/// 从用户输入中提取歌单 ID
fn extract_playlist_id(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("请输入歌单链接或 ID".into());
    }

    if input.chars().all(|c| c.is_ascii_digit()) {
        return Ok(input.to_string());
    }

    let url = Url::parse(input).map_err(|_| "无法识别的歌单链接或 ID".to_string())?;

    if let Some((_, id)) = url.query_pairs().find(|(k, _)| k == "id") {
        let id = id.trim().to_string();
        if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
            return Ok(id);
        }
    }

    if let Some(segments) = url.path_segments() {
        let segs: Vec<&str> = segments.collect();
        if let Some(pos) = segs.iter().position(|s| *s == "playlist") {
            if let Some(id_part) = segs.get(pos + 1) {
                let id = id_part.trim_end_matches(".html");
                if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                    return Ok(id.to_string());
                }
            }
        }
    }

    Err("无法从链接中提取歌单 ID".into())
}

/// 获取歌单歌曲列表
/// https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/songList.js#L196
#[command]
pub async fn fetch_playlist_songs(input: String) -> Result<String, String> {
    let disstid = extract_playlist_id(&input)?;

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

    let cd = data["cdlist"]
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or("未找到歌单数据")?;

    let playlist = json!({
        "id": disstid,
        "name": cd["dissname"].as_str().unwrap_or(""),
        "creator": cd["nickname"].as_str().unwrap_or(""),
        "coverUrl": cd["logo"].as_str().unwrap_or(""),
        "songCount": cd["songnum"].as_u64().unwrap_or(0),
        "playCount": cd["visitnum"].as_u64().unwrap_or(0),
    });

    let songlist = cd["songlist"].as_array().ok_or("未找到歌曲列表")?;
    let mut songs = Vec::new();

    for song in songlist {
        if let Some(song_obj) = parse_song(song) {
            songs.push(song_obj);
        }
    }

    Ok(json!({
        "playlist": playlist,
        "songs": songs
    })
    .to_string())
}

/// 检查 GitHub 最新发布版本
/// 通过 GitHub REST API 获取仓库最新 release 信息，返回 JSON 字符串
/// 字段包含：tag_name、name、body（更新内容）、html_url、published_at、prerelease、current_version
#[command]
pub async fn check_update() -> Result<String, String> {
    let url = "https://api.github.com/repos/lerdb/HotDownloader/releases/latest";

    // 使用全局 CLIENT 发起 GET 请求，并携带 GitHub API 推荐的头信息
    let resp = CLIENT
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 提取关键字段（tag_name 可能带前缀 v，当前版本由 Cargo.toml 编译时提供）
    let tag_name = data["tag_name"].as_str().unwrap_or("").to_string();
    let name = data["name"].as_str().unwrap_or("").to_string();
    let body = data["body"].as_str().unwrap_or("").to_string();
    let html_url = data["html_url"].as_str().unwrap_or("").to_string();
    let published_at = data["published_at"].as_str().unwrap_or("").to_string();
    let prerelease = data["prerelease"].as_bool().unwrap_or(false);

    let result = json!({
        "tag_name": tag_name,
        "name": name,
        "body": body,
        "html_url": html_url,
        "published_at": published_at,
        "prerelease": prerelease,
        "current_version": env!("CARGO_PKG_VERSION")
    });

    Ok(serde_json::to_string(&result).map_err(|e| format!("序列化结果失败: {}", e))?)
}

#[cfg(test)]
mod tests {
    use super::plain_link_error;

    #[test]
    fn maps_platform_block_codes() {
        assert_eq!(
            plain_link_error(104003, ""),
            "无法获取该音质的下载链接（可能需要登录，或该歌曲暂无此音质）"
        );
        assert_eq!(plain_link_error(104004, ""), "该歌曲已下架或禁止下载");
        assert_eq!(plain_link_error(0, ""), "无法获取该音质的下载链接");
    }

    #[test]
    fn keeps_unknown_code_and_optional_tips() {
        assert_eq!(
            plain_link_error(12345, ""),
            "获取下载链接失败，错误码: 12345"
        );
        assert_eq!(
            plain_link_error(12345, "  版权限制  "),
            "获取下载链接失败，错误码: 12345，版权限制"
        );
    }
}

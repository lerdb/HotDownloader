//! 歌曲搜索模块。
//!
//! 通过 QQ 音乐搜索接口进行歌曲搜索，返回解析后的歌曲列表和分页信息。
//! 歌曲解析复用 [`super::parser::parse_song`] 函数。
//!
//! # 接口版本与回退策略
//!
//! QQ 音乐搜索接口存在两套可用的服务端 method：
//!
//! - **Desktop**（`DoSearchForQQMusicDesktop`）：新版接口，桌面端/网页端使用，
//!   响应结构为 `body.song.list`、`body.singer.list`、`body.album.list`。
//! - **Lite**（`DoSearchForQQMusicLite`）：旧版移动端接口，响应结构为
//!   `body.item_song`、`body.singer`、`body.item_album`。
//!
//! 现在以 Desktop 接口为主，但它可能因服务端调整、风控或字段变更而失败。
//! 因此本模块对歌曲、歌手、专辑三类搜索统一采用「先 Desktop，失败后回退 Lite」
//! 的策略：只要有一次成功就返回结果；两次都失败才向上层抛出错误，且错误信息
//! 会同时保留两次失败的原因，便于排查是哪一层出的问题。
//!
//! 注意：**合法的空结果不会触发回退**。只有当请求层（网络、`code != 0`、风控）
//! 或解析层（响应中找不到预期的列表字段）出错时，才会尝试下一个接口版本。

use rand::Rng;
use serde_json::{json, Value};

use super::parser::parse_song;
use crate::platforms::{get_guid, CLIENT};

/// 生成单次搜索请求使用的 `searchid`。
///
/// 格式为 `guid + 5 位随机数字后缀`，用于模拟客户端每次搜索生成的唯一标识，
/// 降低被服务端识别为固定请求模式的风险。
pub(crate) fn get_search_id() -> String {
    let mut rng = rand::rng();
    let suffix: u32 = rng.random_range(0..100_000);
    format!("{}{:05}", get_guid(), suffix)
}

/// QQ 音乐搜索接口版本。
///
/// 不同版本不仅 `method` 不同，`comm` 公共参数和响应字段路径也不同，
/// 因此把「method + comm + 字段路径 + 展示名」聚合在一起，避免在调用处散落分支。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchApiVersion {
    /// 新版 Desktop 接口 `DoSearchForQQMusicDesktop`。
    ///
    /// 响应字段：
    /// - 歌曲：`req.data.body.song.list`
    /// - 歌手：`req.data.body.singer.list`
    /// - 专辑：`req.data.body.album.list`
    ///
    /// 专辑字段名：`albumMID` / `albumName` / `albumPic` / `publicTime` / `song_count`。
    Desktop,

    /// 旧版移动端 Lite 接口 `DoSearchForQQMusicLite`。
    ///
    /// 响应字段：
    /// - 歌曲：`req.data.body.item_song`
    /// - 歌手：`req.data.body.singer`
    /// - 专辑：`req.data.body.item_album`
    ///
    /// 专辑字段名：`albummid` / `name` / `pic` / `description` / `song_num`。
    Lite,
}

impl SearchApiVersion {
    /// 按优先级排列的接口版本，前面的先尝试，失败后依次回退。
    ///
    /// 当前顺序：Desktop → Lite。若将来需要调整主备关系，只需改这里。
    const ALL: [Self; 2] = [Self::Desktop, Self::Lite];

    /// 返回该版本对应的服务端 method 名。
    fn method(self) -> &'static str {
        match self {
            Self::Desktop => "DoSearchForQQMusicDesktop",
            Self::Lite => "DoSearchForQQMusicLite",
        }
    }

    /// 返回该版本对应的 `comm` 公共参数。
    ///
    /// Desktop 与 Lite 对客户端身份、版本号、设备信息的要求不同：
    /// - Desktop 使用较新的客户端标识（`ct=19`、`cv=2151`）；
    /// - Lite 使用旧版移动端设备参数（`ct=11`、`cv=14090508`、机型、ROM 等）。
    fn comm(self) -> Value {
        match self {
            Self::Desktop => desktop_comm(),
            Self::Lite => mobile_comm(),
        }
    }

    /// 版本展示名，用于拼接错误信息，方便定位是哪个接口失败。
    fn name(self) -> &'static str {
        match self {
            Self::Desktop => "Desktop",
            Self::Lite => "Lite",
        }
    }
}

/// 搜索歌曲，返回 JSON 字符串，包含 `songs` 和 `has_more` 两个字段。
///
/// 该命令调用 QQ 音乐搜索接口，根据关键字、页码和每页数量搜索歌曲。
/// 请求中需要动态生成 `searchid` 和 `guid`，并携带设备信息等参数以模拟真实客户端。
/// 搜索结果中的每首歌曲通过 [`parse_song`] 解析。
///
/// # 回退逻辑
///
/// 依次尝试 [`SearchApiVersion::ALL`] 中的接口版本：
/// - 任一版本成功，立即返回其结果；
/// - 全部失败，返回聚合错误，错误信息包含每个版本的失败原因。
///
/// # 参数
/// - `separator`: 多歌手名称拼接时使用的分隔符（透传给 `parse_song`）。
/// - `keyword`: 搜索关键字。
/// - `page`: 页码（从 1 开始）。
/// - `limit`: 每页歌曲数量。
///
/// # 返回
/// - `Ok(String)`：JSON 字符串，包含 `songs`（歌曲数组）和 `has_more`（是否有下一页）。
/// - `Err(String)`：所有接口均失败时的聚合错误信息。
pub async fn search_songs(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let mut errors: Vec<String> = Vec::new();
    for api in SearchApiVersion::ALL {
        match search_songs_with(api, separator, &keyword, page, limit).await {
            Ok(result) => return Ok(result),
            // 记录带版本名的错误，继续尝试下一个版本
            Err(e) => errors.push(format!("{} 接口: {}", api.name(), e)),
        }
    }
    Err(format!(
        "搜索歌曲失败，所有接口均不可用: {}",
        errors.join("; ")
    ))
}

/// 使用指定接口版本执行一次歌曲搜索。
///
/// 该函数只负责单个版本的请求与解析，不处理回退；回退由 [`search_songs`] 负责。
async fn search_songs_with(
    api: SearchApiVersion,
    separator: &str,
    keyword: &str,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = search_request(keyword, page, limit, 0, api).await?;
    let req = &data["req"];

    // 提取歌曲列表：Desktop 走 body.song.list，Lite 走 body.item_song。
    // 字段不存在说明响应结构不符合预期，视为该版本失败，交由上层回退。
    let item_song = match api {
        SearchApiVersion::Desktop => req["data"]["body"]["song"]["list"].as_array(),
        SearchApiVersion::Lite => req["data"]["body"]["item_song"].as_array(),
    }
    .ok_or("未找到歌曲列表")?;

    // 分页判断：
    // 避免因 parse_song 过滤导致有效歌曲数不足一页时，误判为无更多结果，
    // 影响“加载更多”按钮显示。直接读取接口返回的 meta.nextpage 字段，
    // 该字段为 -1 表示无下一页，否则为下一页页码。
    let meta = &req["data"]["meta"];
    let nextpage = meta["nextpage"].as_i64().unwrap_or(-1);
    let has_more = nextpage != -1;

    // 逐首解析歌曲，过滤无法解析的条目
    let mut songs: Vec<Value> = Vec::new();
    for item in item_song {
        if let Some(song_obj) = parse_song(item, separator) {
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

/// 共用搜索请求，向 `musicu.fcg` 发送 POST 并做基础状态校验。
///
/// # 参数
/// - `keyword`: 搜索关键字。
/// - `page`: 页码（从 1 开始）。
/// - `limit`: 每页数量。
/// - `search_type`: 搜索类型，`0` 歌曲、`1` 歌手、`2` 专辑，
///   不同类型返回的响应列表字段不同。
/// - `api`: 使用哪个接口版本（决定 `method` 与 `comm`）。
///
/// # 校验
/// - 顶层 `code` 必须为 0；
/// - `req.code` 必须为 0，其中 `2001` 视为风控错误，单独给出提示。
///
/// 任一校验失败都会返回 `Err`，由调用方的回退逻辑决定是否切换版本。
async fn search_request(
    keyword: &str,
    page: u32,
    limit: u32,
    search_type: u32,
    api: SearchApiVersion,
) -> Result<Value, String> {
    // 构造请求体：
    // - comm：客户端公共参数，随接口版本变化；
    // - req.method：搜索服务 method，随接口版本变化；
    // - req.param：业务参数，两个版本结构一致。
    let request_body = json!({
        "comm": api.comm(),
        "req": {
            "module": "music.search.SearchCgiService",
            "method": api.method(),
            "param": {
                // 0 搜索歌曲，1 搜索歌手，2 搜索专辑
                "search_type": search_type,
                "query": keyword,
                "page_num": page,
                "num_per_page": limit,
                // 每次搜索动态生成，模拟客户端行为
                "searchid": get_search_id(),
                // 以下为客户端固定参数，含义与搜索过滤/高亮/分组有关
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
        .header("Referer", "https://y.qq.com")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 顶层状态校验
    if data["code"] != 0 {
        return Err(format!("接口错误: code={}", data["code"]));
    }

    // 业务层状态校验
    let req = &data["req"];
    if req["code"] != 0 {
        if req["code"] == 2001 {
            // 2001 是 QQ 音乐常见的风控/限流返回码
            return Err("当前搜索被风控，请稍后再试".to_string());
        } else {
            return Err(format!("搜索错误: req.code={}", req["code"]));
        }
    }

    Ok(data)
}

/// 新版 Desktop 接口使用的 `comm` 公共参数。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/323e4bac505da428adae8db6b58039fb2b36b86d/src/renderer/utils/musicSdk/tx/musicSearch.js#L14>
pub(super) fn desktop_comm() -> Value {
    json!({
        "_channelid": "0",
        "_os_version": "6.2.9200-2",
        "ct": "19",
        "cv": "2151",
        "guid": get_guid(),
        "patch": "118",
        "psrf_access_token_expiresAt": 0,
        "psrf_qqaccess_token": "",
        "psrf_qqopenid": "",
        "psrf_qqunionid": "",
        "tmeAppID": "qqmusic",
        "tmeLoginType": 0,
        "uin": "0",
        "wid": "7223299733393904640",
    })
}

/// 旧版移动端 Lite 接口使用的 `comm` 公共参数。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/musicSearch.js#L14>
pub(super) fn mobile_comm() -> Value {
    json!({
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
    })
}

/// 搜索歌手，返回 JSON 字符串，包含 `artists` 和 `has_more`。
///
/// 回退策略与 [`search_songs`] 一致：Desktop → Lite，全部失败才报错。
pub async fn search_artists(keyword: String, page: u32, limit: u32) -> Result<String, String> {
    let mut errors: Vec<String> = Vec::new();
    for api in SearchApiVersion::ALL {
        match search_artists_with(api, &keyword, page, limit).await {
            Ok(result) => return Ok(result),
            Err(e) => errors.push(format!("{} 接口: {}", api.name(), e)),
        }
    }
    Err(format!(
        "搜索歌手失败，所有接口均不可用: {}",
        errors.join("; ")
    ))
}

/// 使用指定接口版本执行一次歌手搜索。
async fn search_artists_with(
    api: SearchApiVersion,
    keyword: &str,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = search_request(keyword, page, limit, 1, api).await?;
    let data = &data["req"]["data"];

    // 歌手列表路径：Desktop 为 body.singer.list，Lite 为 body.singer
    let items = match api {
        SearchApiVersion::Desktop => data["body"]["singer"]["list"].as_array(),
        SearchApiVersion::Lite => data["body"]["singer"].as_array(),
    }
    .ok_or("未找到歌手列表")?;

    let artists: Vec<Value> = items
        .iter()
        .filter_map(super::artist::parse_artist)
        .collect();

    Ok(json!({
        "artists": artists,
        // 列表为空时也视为无更多结果；否则读取 meta.nextpage
        "has_more": !items.is_empty() && data["meta"]["nextpage"].as_i64().unwrap_or(-1) != -1
    })
    .to_string())
}

/// 搜索专辑，返回 JSON 字符串，包含 `albums` 和 `has_more`。
///
/// 回退策略与 [`search_songs`] 一致：Desktop → Lite，全部失败才报错。
/// 两个版本不仅列表路径不同，专辑字段名也不同，具体映射见
/// [`search_albums_with`] 内部说明。
pub async fn search_albums(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let mut errors: Vec<String> = Vec::new();
    for api in SearchApiVersion::ALL {
        match search_albums_with(api, separator, &keyword, page, limit).await {
            Ok(result) => return Ok(result),
            Err(e) => errors.push(format!("{} 接口: {}", api.name(), e)),
        }
    }
    Err(format!(
        "搜索专辑失败，所有接口均不可用: {}",
        errors.join("; ")
    ))
}

/// 使用指定接口版本执行一次专辑搜索。
///
/// # 新旧字段映射
///
/// | 语义       | Desktop 字段   | Lite 字段      |
/// |------------|----------------|----------------|
/// | 专辑 MID   | `albumMID`     | `albummid`     |
/// | 专辑名     | `albumName`    | `name`         |
/// | 封面       | `albumPic`     | `pic`          |
/// | 发行时间   | `publicTime`   | `description`  |
/// | 歌曲数     | `song_count`   | `song_num`     |
///
/// 列表路径：Desktop 为 `body.album.list`，Lite 为 `body.item_album`。
async fn search_albums_with(
    api: SearchApiVersion,
    separator: &str,
    keyword: &str,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = search_request(keyword, page, limit, 2, api).await?;
    let data = &data["req"]["data"];

    // 专辑列表路径随版本变化
    let items = match api {
        SearchApiVersion::Desktop => data["body"]["album"]["list"].as_array(),
        SearchApiVersion::Lite => data["body"]["item_album"].as_array(),
    }
    .ok_or("未找到专辑列表")?;

    // 按版本选择字段名，避免在每个字段上写 match
    let (mid_key, name_key, pic_key, date_key, count_key) = match api {
        SearchApiVersion::Desktop => (
            "albumMID",
            "albumName",
            "albumPic",
            "publicTime",
            "song_count",
        ),
        SearchApiVersion::Lite => ("albummid", "name", "pic", "description", "song_num"),
    };

    let albums: Vec<Value> = items
        .iter()
        .filter_map(|item| {
            // 没有有效 mid 的专辑直接丢弃
            let mid = item[mid_key].as_str().filter(|s| !s.is_empty())?;
            let artists = super::parser::parse_artists(&item["singer_list"]);
            let artist = artists
                .iter()
                .filter_map(|artist| artist["name"].as_str())
                .collect::<Vec<_>>()
                .join(separator);
            Some(json!({
                "id": mid,
                "name": item[name_key].as_str().unwrap_or(""),
                "artist": artist,
                "artists": artists,
                // 封面统一升级为 https
                "coverUrl": item[pic_key].as_str().unwrap_or("").replace("http://", "https://"),
                "publishDate": item[date_key].as_str().unwrap_or(""),
                // 歌曲数兼容数字和字符串两种返回形式
                "songCount": item[count_key]
                    .as_u64()
                    .or_else(|| item[count_key].as_str().and_then(|s| s.parse().ok()))
                    .unwrap_or(0)
            }))
        })
        .collect();

    Ok(json!({
        "albums": albums,
        "has_more": data["meta"]["nextpage"].as_i64().unwrap_or(-1) != -1
    })
    .to_string())
}

//! 歌词获取模块。
//!
//! 通过 QQ 音乐歌曲 ID 获取歌词，支持 QRC 解密、转换为 LRC 和增强 LRC 格式。

use crate::utils::qrc;
use serde::Serialize;
use serde_json::{json, Value};

use crate::platforms::lyric::LyricData;
use crate::utils::http::CLIENT;

/// 歌词接口地址。
const LYRIC_ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";

/// 歌词请求的公共参数。
#[derive(Serialize)]
struct Comm {
    ct: &'static str,
    cv: &'static str,
    uin: &'static str,
}

/// 歌词请求的模块和参数封装。
#[derive(Serialize)]
struct LyricReq {
    method: &'static str,
    module: &'static str,
    param: LyricParam,
}

/// 歌词请求的具体参数。
#[derive(Serialize)]
struct LyricParam {
    crypt: u32,
    ct: u32,
    cv: u32,
    interval: u32,
    lrc_t: u32,
    qrc: u32,
    qrc_t: u32,
    roma: u32,
    roma_t: u32,
    #[serde(rename = "songID")]
    song_id: u64,
    trans: u32,
    trans_t: u32,
    #[serde(rename = "type")]
    type_: i32,
}

impl LyricParam {
    /// 根据歌曲 ID 创建默认歌词请求参数。
    fn new(song_id: u64) -> Self {
        Self {
            crypt: 1,
            ct: 19,
            cv: 1873,
            interval: 0,
            lrc_t: 0,
            qrc: 1,
            qrc_t: 0,
            roma: 0,
            roma_t: 0,
            song_id,
            trans: 0,
            trans_t: 0,
            type_: -1,
        }
    }
}

/// 通过 QQ 音乐歌曲 ID 获取歌词。
///
/// 返回 [`LyricData`]，包含普通 LRC 和增强 LRC（若 QRC 可用）。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/lyric.js#L230>
///
/// # 参数
/// - `song_id`: QQ 音乐歌曲的数字 ID。
///
/// # 返回
/// - `Ok(LyricData)`：包含歌词信息的结构体。
/// - `Err(String)`：错误信息，如请求失败、接口错误、解密失败等。
pub(crate) async fn get_lyric_by_id(song_id: u64) -> Result<LyricData, String> {
    // 构造请求体
    let body = json!({
        "comm": Comm {
            ct: "19",
            cv: "1859",
            uin: "0",
        },
        "req": LyricReq {
            method: "GetPlayLyricInfo",
            module: "music.musichallSong.PlayLyricInfo",
            param: LyricParam::new(song_id),
        },
    });

    // 发送 POST 请求
    let response = CLIENT
        .post(LYRIC_ENDPOINT)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("歌词请求失败: {e}"))?;

    // 检查 HTTP 状态码
    if !response.status().is_success() {
        return Err(format!("歌词接口返回 HTTP {}", response.status()));
    }

    // 解析响应 JSON
    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("解析歌词响应失败: {e}"))?;

    // 检查网关和模块返回码
    if json["code"] != 0 {
        let code = json["code"].as_i64().unwrap_or(-1);
        return Err(format!("歌词网关错误码 {code}"));
    }
    if json["req"]["code"] != 0 {
        let code = json["req"]["code"].as_i64().unwrap_or(-1);
        return Err(format!("歌词模块错误码 {code}"));
    }

    // 提取加密歌词字段（可能为空）
    let encrypted = json["req"]["data"]["lyric"].as_str().unwrap_or("").trim();

    if encrypted.is_empty() {
        return Ok(LyricData {
            lrc: None,
            elrc: None,
            raw: None,
            instrumental: false,
        });
    }

    // 解密 QRC（十六进制 -> 自定义 3DES -> zlib -> XML）
    let xml = qrc::decrypt(encrypted).map_err(|e| format!("歌词解密失败: {e}"))?;
    // 提取 <LyricContent> 中的原始歌词内容
    let raw_content =
        qrc::extract_lyric_content(&xml).ok_or_else(|| "解密后未找到 LyricContent".to_string())?;

    // 判断是否为 QRC 格式
    let is_qrc = qrc::is_qrc(&raw_content);

    // 生成普通 LRC
    let lrc = if is_qrc {
        Some(qrc::to_lrc(&raw_content))
    } else {
        Some(raw_content.clone()) // 纯 LRC
    };

    // 生成增强 LRC（仅 QRC 格式可用）
    let elrc = if is_qrc {
        Some(qrc::to_enhanced_lrc(&raw_content))
    } else {
        None
    };

    // 简单的纯音乐检测（可选）
    let instrumental = lrc
        .as_ref()
        .map(|s| s.contains("纯音乐") || s.contains("Instrumental"))
        .unwrap_or(false);

    Ok(LyricData {
        lrc,
        elrc,
        raw: Some(raw_content),
        instrumental,
    })
}

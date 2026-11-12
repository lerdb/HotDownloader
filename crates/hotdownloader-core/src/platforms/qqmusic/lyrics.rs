//! 歌词获取模块。
//!
//! 通过 QQ 音乐歌曲 ID 获取歌词，支持 QRC 解密、转换为 LRC 和增强 LRC 格式。

use crate::platforms::qqmusic::qrc;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

use crate::platforms::lyric::LyricData;
use crate::platforms::CLIENT;

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
pub async fn get_lyric_by_id(song_id: u64) -> Result<LyricData, String> {
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

    parse_response(&json)
}

fn parse_response(json: &Value) -> Result<LyricData, String> {
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
    let encrypted = json["req"]["data"]["lyric"]
        .as_str()
        .ok_or("歌词响应格式异常：缺少 lyric 字符串")?
        .trim();

    if encrypted.is_empty() {
        return Ok(LyricData::default());
    }

    // 解密 QRC（十六进制 -> 自定义 3DES -> zlib -> XML）
    let decrypted = qrc::decrypt(encrypted).map_err(|e| format!("歌词解密失败: {e}"))?;
    parse_decrypted_lyric(&decrypted)
}

fn is_instrumental_notice(text: &str) -> bool {
    matches!(
        text.trim().trim_end_matches(['。', '.', '！', '!']),
        "纯音乐，请欣赏" | "此歌曲为没有填词的纯音乐，请您欣赏" | "此歌曲为纯音乐，请您欣赏"
    ) || text.trim().eq_ignore_ascii_case("instrumental")
}

pub(crate) fn parse_decrypted_lyric(decrypted: &str) -> Result<LyricData, String> {
    let text = decrypted.trim();
    if text.is_empty() {
        return Ok(LyricData::default());
    }
    let line_time = Regex::new(r"^\[\d+:\d{2}(?:[.:]\d+)?\](.*)$").unwrap();
    let raw_content = if let Some(content) = qrc::extract_lyric_content(text) {
        content
    } else if !text.starts_with('<')
        && (qrc::is_qrc(text) || text.lines().any(|line| line_time.is_match(line.trim())))
    {
        // 接口也会返回未包在 XML 中的普通 LRC，包括纯音乐说明。
        text.to_string()
    } else if is_instrumental_notice(text) {
        format!("[00:00.00]{text}")
    } else {
        return Err("歌词响应格式异常：未找到 LyricContent 或有效的时间轴歌词".into());
    };
    if raw_content.trim().is_empty() {
        return Ok(LyricData::default());
    }

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

    // 只将独立的纯音乐说明行当作标记，不根据歌名或歌词中的关键词推断。
    let instrumental = lrc.as_ref().is_some_and(|lrc| {
        lrc.lines().any(|line| {
            line_time
                .captures(line.trim())
                .is_some_and(|capture| is_instrumental_notice(&capture[1]))
        })
    });

    Ok(LyricData {
        lrc,
        elrc,
        raw: Some(raw_content),
        instrumental,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(plaintext: &str) -> Value {
        json!({"code":0,"req":{"code":0,"data":{"lyric":qrc::encrypt_test_payload(plaintext)}}})
    }

    #[test]
    fn successful_empty_fields_and_empty_xml_mean_lyrics_are_unavailable() {
        for value in [
            json!({"code":0,"req":{"code":0,"data":{"lyric":""}}}),
            response(""),
            response(r#"<QrcInfos><Lyric_1 LyricContent=""/></QrcInfos>"#),
            response("<QrcInfos><Lyric_1 LyricContent = '  ' LyricType='1'/></QrcInfos>"),
        ] {
            let lyric = parse_response(&value).unwrap();
            assert!(lyric.lrc.is_none());
            assert!(!lyric.instrumental);
        }
    }

    #[test]
    fn decrypted_plain_instrumental_lrc_and_xml_are_both_valid() {
        for text in [
            "[ti:Oblivion]\n[00:00.00]此歌曲为没有填词的纯音乐，请您欣赏",
            r#"<QrcInfos><Lyric_1 LyricContent="[00:00.00]纯音乐，请欣赏" LyricType="1"/></QrcInfos>"#,
            "纯音乐，请欣赏",
        ] {
            let lyric = parse_response(&response(text)).unwrap();
            assert!(lyric.instrumental);
            assert!(lyric.lrc.unwrap().contains("纯音乐"));
        }
        let lyric = parse_response(&response(
            "[00:01.00]虚构歌词中的 Instrumental 不是类型标记",
        ))
        .unwrap();
        assert!(!lyric.instrumental);
        let lyric = parse_response(&response(
            r#"<QrcInfos><Lyric_1 LyricContent="[0,1000]虚构歌词(0,1000)"/></QrcInfos>"#,
        ))
        .unwrap();
        assert_eq!(lyric.lrc.as_deref(), Some("[00:00.00]虚构歌词"));
        assert!(lyric.elrc.is_some());
    }

    #[test]
    fn missing_lyric_content_does_not_prove_an_instrumental_track() {
        for value in [
            json!({"code":0,"req":{"code":0,"data":{}}}),
            json!({"code":0,"req":{"code":0,"data":{"lyric":null}}}),
            json!({"code":0,"req":{"code":0,"data":{"lyric":"not encrypted hex"}}}),
            json!({"code":0,"req":{"code":429,"data":{"lyric":""}}}),
            response("<QrcInfos><Lyric_1/></QrcInfos>"),
            response("upstream error, no lyric content"),
        ] {
            assert!(parse_response(&value).is_err(), "{value}");
        }
        let error = parse_response(&response("<QrcInfos><Lyric_1/></QrcInfos>")).unwrap_err();
        assert!(error.starts_with("歌词响应格式异常"));
        assert!(!error.contains("解密失败"));
    }
}

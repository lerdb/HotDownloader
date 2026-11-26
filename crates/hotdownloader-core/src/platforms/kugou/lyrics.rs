use super::{client::request_with, parser::valid_hash};
use crate::platforms::{
    lyric::{format_lyric_time, AuxiliaryLine, LyricAnchor, LyricData, LyricTracks},
    CLIENT,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::read::ZlibDecoder;
use regex::Regex;
use serde_json::{json, Value};
use std::{future::Future, io::Read};

// KRC 帧格式参考 MakcRe/KuGouMusicApi 的 MIT 实现（util/util.js）。
const KRC_KEY: [u8; 16] = *b"@Gaw^2tGQ61-\xce\xd2ni";

pub async fn get_lyric(hash: &str) -> Result<LyricData, String> {
    Ok(get_lyric_tracks(hash, false).await?.original)
}

pub(crate) async fn get_lyric_tracks(hash: &str, prefer_krc: bool) -> Result<LyricTracks, String> {
    if !valid_hash(hash) {
        return Err("无效的酷狗歌曲 hash".into());
    }
    let v = request_with(
        &CLIENT,
        "https://lyrics.kugou.com",
        "/v1/search",
        "",
        json!({
            "hash": hash,
            "album_audio_id": 0,
            "duration": 0,
            "keyword": "",
            "lrctxt": 1,
            "man": "no"
        }),
        None,
    )
    .await?;
    let Some(candidate) = v["candidates"].as_array().and_then(|v| v.first()) else {
        return Ok(LyricTracks::default());
    };
    fetch_candidate(prefer_krc, |format| download_content(candidate, format)).await
}

async fn download_content(candidate: &Value, format: &str) -> Result<Vec<u8>, String> {
    let v = request_with(
        &CLIENT,
        "https://lyrics.kugou.com",
        "/download",
        "",
        json!({
            "ver": 1,
            "client": "android",
            "id": candidate["id"],
            "accesskey": candidate["accesskey"],
            "fmt": format,
            "charset": "utf8"
        }),
        None,
    )
    .await?;
    let content = v["content"].as_str().ok_or("酷狗歌词响应缺少正文")?;
    STANDARD
        .decode(content)
        .map_err(|e| format!("解码酷狗歌词失败: {e}"))
}

async fn fetch_candidate<F, Fut>(prefer_krc: bool, mut fetch: F) -> Result<LyricTracks, String>
where
    F: FnMut(&'static str) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    if prefer_krc {
        match fetch("krc").await.and_then(|bytes| parse_krc(&bytes)) {
            Ok(lyric) => return Ok(lyric),
            Err(error) => log::debug!("[酷狗] KRC 不可用，回退 LRC: {error}"),
        }
    }
    let bytes = fetch("lrc").await?;
    let lrc = String::from_utf8(bytes).map_err(|e| format!("解析酷狗歌词失败: {e}"))?;
    Ok(LyricData {
        lrc: Some(lrc.clone()),
        raw: Some(lrc),
        ..Default::default()
    }
    .into())
}

fn parse_krc(bytes: &[u8]) -> Result<LyricTracks, String> {
    let encrypted = bytes.strip_prefix(b"krc1").ok_or("酷狗 KRC 文件头无效")?;
    let compressed: Vec<_> = encrypted
        .iter()
        .zip(KRC_KEY.iter().cycle())
        .map(|(byte, key)| byte ^ key)
        .collect();
    let mut decoded = String::new();
    ZlibDecoder::new(compressed.as_slice())
        .read_to_string(&mut decoded)
        .map_err(|error| format!("解压酷狗 KRC 失败: {error}"))?;
    parse_krc_text(&decoded)
}

fn parse_krc_text(decoded: &str) -> Result<LyricTracks, String> {
    let line_tag = Regex::new(r"^\[([0-9]+),[0-9]+\](.*)$").unwrap();
    let word_tag = Regex::new(r"<([0-9]+),([0-9]+),[0-9]+>").unwrap();
    let mut lrc = Vec::new();
    let mut elrc = Vec::new();
    let mut source_indices = Vec::new();
    let mut source_line_count = 0;
    let mut has_lyric = false;
    let mut language = None;
    for raw in decoded.trim_start_matches('\u{feff}').lines() {
        if let Some(encoded) = raw
            .strip_prefix("[language:")
            .and_then(|s| s.strip_suffix(']'))
        {
            language = STANDARD
                .decode(encoded)
                .ok()
                .and_then(|json| serde_json::from_slice::<Value>(&json).ok());
            continue;
        }
        let Some(line) = line_tag.captures(raw) else {
            if raw.starts_with('[') && raw.ends_with(']') {
                lrc.push(raw.to_owned());
                elrc.push(raw.to_owned());
            }
            continue;
        };
        let source_index = source_line_count;
        source_line_count += 1;
        let Ok(start) = line[1].parse::<u64>() else {
            continue;
        };
        let body = &line[2];
        let time = format!("[{}]", format_lyric_time(start));
        let plain = word_tag.replace_all(body, "");
        let mut enhanced = String::new();
        let mut end = None;
        let mut words = word_tag.captures_iter(body).peekable();
        if let Some(first) = words.peek() {
            enhanced.push_str(&body[..first.get(0).unwrap().start()]);
        }
        while let Some(word) = words.next() {
            let Some((position, word_end)) = krc_word_time(start, &word) else {
                enhanced.clear();
                end = None;
                break;
            };
            let tail = words
                .peek()
                .map_or(body.len(), |next| next.get(0).unwrap().start());
            enhanced.push_str(&format!("<{}>", format_lyric_time(position)));
            enhanced.push_str(&body[word.get(0).unwrap().end()..tail]);
            end = Some(word_end);
        }
        has_lyric |= !plain.trim().is_empty();
        lrc.push(format!("{time}{plain}"));
        if let Some(end) = end.filter(|end| *end > start && !plain.trim().is_empty()) {
            elrc.push(format!("{time}{enhanced}<{}>", format_lyric_time(end)));
        } else {
            elrc.push(format!("{time}{plain}"));
        }
        source_indices.push(source_index);
    }
    if !has_lyric {
        return Err("酷狗 KRC 缺少有效歌词".into());
    }
    let mut tracks = LyricTracks::from(LyricData {
        lrc: Some(lrc.join("\n")),
        elrc: Some(elrc.join("\n")),
        ..Default::default()
    });
    if let Some(layers) = language
        .as_ref()
        .and_then(|value| value["content"].as_array())
    {
        for layer in layers {
            let output = match layer["type"].as_u64() {
                Some(0) => &mut tracks.romanization,
                Some(1) => &mut tracks.translation,
                _ => continue,
            };
            if let Some(rows) = layer["lyricContent"].as_array() {
                for (index, &source_index) in source_indices.iter().enumerate() {
                    if let Some(text) =
                        rows.get(source_index)
                            .and_then(Value::as_array)
                            .and_then(|chunks| {
                                chunks.iter().map(Value::as_str).collect::<Option<String>>()
                            })
                    {
                        output.push(AuxiliaryLine {
                            anchor: LyricAnchor::Line(index),
                            text,
                        });
                    }
                }
            }
        }
    }
    Ok(tracks)
}

fn krc_word_time(start: u64, word: &regex::Captures<'_>) -> Option<(u64, u64)> {
    let position = start.checked_add(word[1].parse().ok()?)?;
    let end = position.checked_add(word[2].parse().ok()?)?;
    Some((position, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    #[test]
    fn krc_lyric_tracks_keep_source_indices_and_convert_relative_word_times() {
        let language = STANDARD.encode(
            json!({
                "content": [
                    {"type": 0, "lyricContent": [["yo", "ru"], [], ["so", "ra"]]},
                    {"type": 1, "lyricContent": [["夜晚"], [], ["天空"]]}
                ]
            })
            .to_string(),
        );
        let raw = format!(
            "[ar:测试]\n[language:{language}]\n[1001,600]<0,211,0>夜<211,389,0>空\n[1701,0]\n[2010,100]<0,100,0>空"
        );
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(raw.as_bytes()).unwrap();
        let mut fixture = b"krc1".to_vec();
        fixture.extend(
            encoder
                .finish()
                .unwrap()
                .into_iter()
                .zip(KRC_KEY.into_iter().cycle())
                .map(|(byte, key)| byte ^ key),
        );
        let tracks = parse_krc(&fixture).unwrap();
        assert_eq!(
            tracks.render(true, true, true).as_deref(),
            Some("[ar:测试]\n[00:01.001]<00:01.001>夜<00:01.212>空<00:01.601>\n[00:01.001]夜晚\n[00:01.001]yoru\n[00:01.701]\n[00:02.010]<00:02.010>空<00:02.110>\n[00:02.010]天空\n[00:02.010]sora")
        );
        assert_eq!(
            parse_krc_text("[language:invalid]\n[1000,100]<0,100,0>原文")
                .unwrap()
                .render(true, true, true)
                .as_deref(),
            Some("[00:01.000]<00:01.000>原文<00:01.100>")
        );
    }

    #[tokio::test]
    async fn unavailable_krc_lyric_falls_back_to_plain_lrc() {
        let mut formats = Vec::new();
        let tracks = fetch_candidate(true, |format| {
            formats.push(format);
            async move {
                Ok(if format == "krc" {
                    b"not a KRC".to_vec()
                } else {
                    "[00:01.00]原文".as_bytes().to_vec()
                })
            }
        })
        .await
        .unwrap();
        assert_eq!(formats, ["krc", "lrc"]);
        assert_eq!(
            tracks.render(true, true, true).as_deref(),
            Some("[00:01.00]原文")
        );
    }
}

//! 普通歌词、逐字歌词转换和纯音乐识别。
use super::client::request;
use crate::platforms::lyric::{auxiliary_lrc, format_lyric_time, LyricData, LyricTracks};
use serde_json::{json, Value};

pub async fn get_lyric_by_id(song_id: u64) -> Result<LyricData, String> {
    let data = request_lyric(song_id, false, false).await?;
    Ok(parse_lyric(&data, false))
}

pub(crate) async fn get_lyric_tracks(
    song_id: u64,
    translation: bool,
    romanization: bool,
) -> Result<LyricTracks, String> {
    let data = request_lyric(song_id, translation, romanization).await?;
    Ok(parse_tracks(&data))
}

async fn request_lyric(
    song_id: u64,
    translation: bool,
    romanization: bool,
) -> Result<Value, String> {
    if song_id == 0 {
        return Err("无效的网易云歌曲 ID".into());
    }
    let data = request(
        "/api/song/lyric/v1",
        json!({
            "id": song_id,
            "cp": false,
            "tv": if translation { -1 } else { 0 },
            "lv": 0,
            "rv": if romanization { -1 } else { 0 },
            "kv": 0,
            "yv": 0,
            "ytv": 0,
            "yrv": 0
        }),
        "",
        true,
    )
    .await?
    .0;
    Ok(data)
}

fn lyric_text<'a>(data: &'a Value, key: &str) -> Option<&'a str> {
    data[key]["lyric"].as_str().filter(|s| !s.trim().is_empty())
}

fn parse_tracks(data: &Value) -> LyricTracks {
    LyricTracks {
        original: parse_lyric(data, true),
        translation: lyric_text(data, "tlyric")
            .map(auxiliary_lrc)
            .unwrap_or_default(),
        romanization: lyric_text(data, "romalrc")
            .map(auxiliary_lrc)
            .unwrap_or_default(),
    }
}

fn parse_lyric(data: &Value, include_end_times: bool) -> LyricData {
    let lrc = lyric_text(data, "lrc").map(str::to_string);
    let yrc = lyric_text(data, "yrc");
    let instrumental = data["nolyric"].as_bool().unwrap_or(false)
        || lrc.as_deref().is_some_and(|s| s.contains("纯音乐，请欣赏"));
    let elrc = yrc
        .map(|raw| yrc_to_elrc(raw, include_end_times))
        .filter(|s| !s.is_empty());
    LyricData {
        lrc,
        elrc,
        raw: yrc.map(str::to_string),
        instrumental,
    }
}

fn yrc_to_elrc(raw: &str, include_end_times: bool) -> String {
    let line = regex::Regex::new(r"^\[([0-9]+),[0-9]+\]").unwrap();
    let word = regex::Regex::new(r"\(([0-9]+),([0-9]+),[0-9]+\)").unwrap();
    raw.lines()
        .filter_map(|text| {
            let header = line.captures(text)?;
            let mut output = format!("[{}]", format_lyric_time(header[1].parse().ok()?));
            let body = &text[header[0].len()..];
            let mut words = word.captures_iter(body).peekable();
            if include_end_times {
                let prefix_end = words
                    .peek()
                    .map_or(body.len(), |word| word.get(0).unwrap().start());
                output.push_str(&body[..prefix_end]);
            }
            let mut last_end = None;
            while let Some(part) = words.next() {
                let start = part[1].parse::<u64>().ok()?;
                let tail = words
                    .peek()
                    .map_or(body.len(), |next| next.get(0).unwrap().start());
                output.push_str(&format!("<{}>", format_lyric_time(start)));
                output.push_str(&body[part.get(0).unwrap().end()..tail]);
                last_end = start.checked_add(part[2].parse().ok()?);
            }
            if let Some(end) = last_end.filter(|_| include_end_times) {
                output.push_str(&format!("<{}>", format_lyric_time(end)));
            }
            Some(output)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_and_translation_preserve_absolute_yrc_word_times() {
        let raw = "[0,1000](0,500,0)First (500,500,0)line\n[0,1000](0,1000,0)第一行\n[1000,1000](1000,500,0)Second (1500,500,0)line\n[1000,1000](1000,1000,0)第二行";
        assert_eq!(yrc_to_elrc(raw, false), "[00:00.000]<00:00.000>First <00:00.500>line\n[00:00.000]<00:00.000>第一行\n[00:01.000]<00:01.000>Second <00:01.500>line\n[00:01.000]<00:01.000>第二行");
    }

    #[test]
    fn lyric_converts_word_times_to_enhanced_lrc() {
        let lyric = parse_lyric(
            &json!({
                "lrc": {"lyric": "[00:01.000]你好"},
                "yrc": {"lyric": "[1000,1000](1000,500,0)你(1500,500,0)好"}
            }),
            false,
        );
        assert_eq!(
            lyric.elrc.as_deref(),
            Some("[00:01.000]<00:01.000>你<00:01.500>好")
        );
        assert!(!lyric.instrumental);
    }

    #[test]
    fn lyric_tracks_bridge_line_and_word_timelines_and_keep_last_word_end() {
        let tracks = parse_tracks(&json!({
            "lrc": {"lyric": "[by:fixture]\n[00:01.00]星空 (夜)\n[00:01.32]月光"},
            "yrc": {"lyric": "[1320,600](1320,300,0)星空 (1620,300,0)(夜)\n[2100,200](2100,200,0)月光"},
            "tlyric": {"lyric": "[00:01.000]Starry night\n[00:01.320]Moonlight"},
            "romalrc": {"lyric": "[00:01.000]Xingkong"}
        }));
        assert_eq!(
            tracks.render(true, true, true).as_deref(),
            Some("[00:01.320]<00:01.320>星空 <00:01.620>(夜)<00:01.920>\n[00:01.320]Starry night\n[00:01.320]Xingkong\n[00:02.100]<00:02.100>月光<00:02.300>\n[00:02.100]Moonlight")
        );
        assert_eq!(
            tracks.render(false, true, false).as_deref(),
            Some("[by:fixture]\n[00:01.00]星空 (夜)\n[00:01.00]Starry night\n[00:01.32]月光\n[00:01.32]Moonlight")
        );
    }
}

//! 酷我音乐歌词解密与格式化工具模块。
//!
//! 提供响应体解密、歌词文本解析、LRC/ELRC 格式化等功能。
//!
//! 1. `decode_lyric`：将酷我歌词接口返回的原始字节解密为 GB18030 文本。
//! 2. `parse_lyric`：解析文本，提取头部标签、普通歌词行和逐字歌词行。
//! 3. `build_lrc_text` / `build_enhanced_lrc_text`：根据解析结果生成最终 LRC/ELRC 字符串。

use base64::{engine::general_purpose::STANDARD, Engine as _};
use flate2::read::ZlibDecoder;
use regex::Regex;
use std::io::Read;

/// XOR 加密密钥，酷我歌词接口固定使用。
const BUF_KEY: &[u8; 7] = b"yeelion";

/// 歌词解析结果中普通歌词行的结构。
#[derive(Debug, Clone)]
pub(crate) struct LyricLine {
    pub time: String,
    pub text: String,
}

/// 歌词解析结果中增强歌词行（逐字）的结构。
#[derive(Debug, Clone)]
pub(crate) struct EnhancedLine {
    /// 源行时间，译文配对时与普通歌词使用同一条时间轴。
    pub time: String,
    pub line_start_ms: i64,
    pub words: Vec<Word>,
    /// 是否具有可供逐字高亮的真实时间；全零词标签只提供行级时间。
    pub has_word_timing: bool,
    pub is_translation: bool,
    /// 全零词标签的源行，可能是译文占位或停唱空行，须结合相邻行判断用途。
    pub is_timing_placeholder: bool,
}

/// 增强歌词行中的单个词。
#[derive(Debug, Clone)]
pub(crate) struct Word {
    pub text: String,
    pub start_ms: i64,
    pub duration_ms: i64,
}

/// 解密酷我歌词响应体。
///
/// 酷我歌词响应格式：以 `tp=content` 开头，之后是 `\r\n\r\n` 分隔符，分隔符后为 zlib 压缩数据。
/// 解压后，如果是逐字歌词模式（`is_get_lyricx = true`），还需进行一次 Base64 解码和 XOR 解密，
/// 最终得到 GB18030 编码的歌词文本。
///
/// 参考实现：<https://github.com/emoeem/voicefox/blob/704a97bffffbc2f5cbedae9f91ba65abb5a26897/source/src/kw/lyric.rs#L58>
///
/// # 参数
/// - `data`: 原始 HTTP 响应字节。
/// - `is_get_lyricx`: 是否按逐字歌词模式解密。
///
/// # 返回
/// 解密后的 UTF-8 字符串。任一步骤失败返回空字符串。
pub(crate) fn decode_lyric(data: &[u8], is_get_lyricx: bool) -> String {
    if !data.starts_with(b"tp=content") {
        return String::new();
    }

    // 查找响应头与压缩数据的分隔符 "\r\n\r\n"
    let separator = b"\r\n\r\n";
    let sep_index = match data.windows(separator.len()).position(|w| w == separator) {
        Some(i) => i,
        None => return String::new(),
    };

    let compressed = &data[sep_index + separator.len()..];
    // zlib 解压
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::new();
    if decoder.read_to_end(&mut decompressed).is_err() {
        return String::new();
    }

    if !is_get_lyricx {
        // 非逐字模式：解压结果直接是 GB18030 文本
        return decode_gb18030(&decompressed);
    }

    // 逐字模式：解压结果是 Base64 编码的加密数据
    let decoded = match STANDARD.decode(&decompressed) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };

    // XOR 解密
    let mut output = Vec::with_capacity(decoded.len());
    let key_len = BUF_KEY.len();
    let mut i = 0;
    while i < decoded.len() {
        let mut j = 0;
        while j < key_len && i < decoded.len() {
            output.push(decoded[i] ^ BUF_KEY[j]);
            i += 1;
            j += 1;
        }
    }

    decode_gb18030(&output)
}

/// 将 GB18030 字节解码为 Rust 字符串。
///
/// 使用 `encoding_rs` 库进行解码，失败时返回空字符串。
fn decode_gb18030(bytes: &[u8]) -> String {
    let (text, _, had_errors) = encoding_rs::GB18030.decode(bytes);
    if had_errors {
        log::warn!("酷我歌词 GB18030 解码存在部分错误，已忽略");
    }
    text.into_owned()
}

/// 解析歌词头部标签（如 `[ver:...]`、`[ti:...]` 等）。
///
/// 特殊处理 `[kuwo:...]` 标签：其值包含八进制偏移量，
/// 用于计算逐字时间的缩放因子。
///
/// # 参数
/// - `lrc`: 原始歌词文本。
/// - `offset`: 输出参数，逐字时间偏移量 1（分子）。
/// - `offset2`: 输出参数，逐字时间偏移量 2（分母）。
///
/// # 返回
/// 保留的非 `kuwo` 标签行列表（用于输出 LRC/ELRC 头部）。
fn parse_tags(lrc: &str, offset: &mut i64, offset2: &mut i64) -> Vec<String> {
    let tag_re = Regex::new(r"\[(ver|ti|ar|al|offset|by|kuwo):\s*(\S+(?:\s+\S+)*)\s*\]").unwrap();
    let mut tags = Vec::new();

    for line in lrc.lines() {
        let line = line.trim();
        if let Some(caps) = tag_re.captures(line) {
            let key = &caps[1];
            let value = caps[2].trim();
            if key == "kuwo" {
                // value 形如 "075540][..."，取第一个 "][" 之前的部分
                let numeric = value.split("][").next().unwrap_or("");
                // 八进制解析为整数
                if let Ok(num) = i64::from_str_radix(numeric, 8) {
                    *offset = num / 10;
                    *offset2 = num % 10;
                }
            } else {
                tags.push(line.to_string());
            }
        }
    }
    tags
}

/// 将时间字符串（如 `[01:23.45]`）转换为秒（浮点数）。
fn time_to_seconds(time_str: &str) -> f64 {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return 0.0;
    }
    let minutes: f64 = parts[0].parse().unwrap_or(0.0);
    let sec_parts: Vec<&str> = parts[1].split('.').collect();
    let seconds: f64 = sec_parts[0].parse().unwrap_or(0.0);
    let millis: f64 = if sec_parts.len() > 1 {
        let ms_str = sec_parts[1];
        let ms_str = if ms_str.len() >= 3 {
            &ms_str[..3]
        } else {
            ms_str
        };
        let ms_str = format!("{:0<3}", ms_str);
        ms_str.parse().unwrap_or(0.0)
    } else {
        0.0
    };
    minutes * 60.0 + seconds + millis / 1000.0
}

/// 将毫秒转换为 `mm:ss.ccc` 格式（三位毫秒）。
pub(crate) fn format_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    let minutes = ms / 60000;
    let seconds = (ms % 60000) / 1000;
    let millis = ms % 1000;
    format!("{:02}:{:02}.{:03}", minutes, seconds, millis)
}

/// 解析歌词正文，提取普通歌词行和逐字歌词行。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/util.js#L123>
///
/// # 参数
/// - `lrc`: 原始歌词文本（已去除头部标签）。
/// - `offset`: 逐字偏移量 1。
/// - `offset2`: 逐字偏移量 2。
///
/// # 返回
/// `(普通歌词行列表, 增强歌词行列表)`。
fn parse_lyric_lines(lrc: &str, offset: i64, offset2: i64) -> (Vec<LyricLine>, Vec<EnhancedLine>) {
    let time_exp = Regex::new(r"^\[([\d:.]*)]").unwrap();
    let word_tag = Regex::new(r"<-?\d+,-?\d+(?:,-?\d+)?>").unwrap();
    let norm_time = Regex::new(r"\.\d\d$").unwrap();

    let mut lyric_lines = Vec::new();
    let mut enhanced_lines = Vec::new();

    for raw_line in lrc.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let Some(caps) = time_exp.captures(line) else {
            continue;
        };
        let time_str = caps.get(1).unwrap().as_str().to_string();
        // 标准化毫秒部分为三位（不足补零）
        let normalized_time = if norm_time.is_match(&time_str) {
            format!("{}0", time_str)
        } else {
            time_str.clone()
        };

        let line_seconds = time_to_seconds(&normalized_time);
        let line_start_ms = (line_seconds * 1000.0).round() as i64;

        // 去掉时间戳后的剩余部分
        let body = time_exp.replace(line, "").trim().to_string();

        // 提取所有逐字标签 <x,y>
        let tag_matches: Vec<_> = word_tag.find_iter(&body).collect();

        if tag_matches.is_empty() {
            // 无逐字标签：整行作为一个词
            let text = body.clone();
            lyric_lines.push(LyricLine {
                time: normalized_time.clone(),
                text: text.clone(),
            });
            enhanced_lines.push(EnhancedLine {
                time: normalized_time.clone(),
                line_start_ms,
                words: vec![Word {
                    text,
                    start_ms: line_start_ms,
                    duration_ms: 0,
                }],
                has_word_timing: false,
                is_translation: false,
                is_timing_placeholder: false,
            });
            continue;
        }

        // 有逐字标签：解析每个词
        let mut words = Vec::new();
        for (idx, m) in tag_matches.iter().enumerate() {
            let tag_str = m.as_str();
            let inner = &tag_str[1..tag_str.len() - 1]; // 去掉 < >
            let parts: Vec<&str> = inner.split(',').collect();
            let x: i64 = parts[0].parse().unwrap_or(0);
            let y: i64 = parts[1].parse().unwrap_or(0);

            let text_start = m.end();
            let text_end = if idx + 1 < tag_matches.len() {
                tag_matches[idx + 1].start()
            } else {
                body.len()
            };
            let word_text = body[text_start..text_end].to_string();

            let relative_start = (x + y) as f64 / (2.0 * offset as f64);
            let duration = (x - y).abs() as f64 / (2.0 * offset2 as f64);

            let absolute_start = line_start_ms + relative_start.round() as i64;
            let duration_ms = duration.round() as i64;

            words.push(Word {
                text: word_text,
                start_ms: absolute_start,
                duration_ms,
            });
        }

        let plain_text: String = words.iter().map(|w| w.text.as_str()).collect();
        let has_word_timing = words
            .iter()
            .any(|word| word.start_ms != line_start_ms || word.duration_ms != 0);
        // 先保留零值空行，待完整时间轴可见后再区分译文占位与间奏/结束标记。
        lyric_lines.push(LyricLine {
            time: normalized_time.clone(),
            text: plain_text,
        });

        let line_abs_start = words.first().map(|w| w.start_ms).unwrap_or(line_start_ms);
        enhanced_lines.push(EnhancedLine {
            time: normalized_time.clone(),
            line_start_ms: line_abs_start,
            words,
            has_word_timing,
            is_translation: false,
            is_timing_placeholder: !has_word_timing,
        });
    }

    // 只有与下一条逐字原文同刻的零值空行，才确定是无内容的译文占位。
    // 倒序同步移除两种表示中的占位；其余空行保留原时间，继续用于清除高亮。
    for index in (0..lyric_lines.len().saturating_sub(1)).rev() {
        if enhanced_lines[index].is_timing_placeholder
            && lyric_lines[index].text.trim().is_empty()
            && enhanced_lines[index + 1].has_word_timing
            && time_to_seconds(&enhanced_lines[index].time)
                == time_to_seconds(&enhanced_lines[index + 1].time)
        {
            lyric_lines.remove(index);
            enhanced_lines.remove(index);
        }
    }
    align_translations(&mut lyric_lines, &mut enhanced_lines);
    (lyric_lines, enhanced_lines)
}

/// 酷我将译文放在原文之后，但常用下一句原文的时间作为译文行时间。
/// 以“有实际逐字时间的原文 + 零值逐字占位译文 + 下一原文同刻”配对，
/// 同时修正 LRC 与 ELRC；原文的逐字时间、行顺序和结束空行均保留。
fn align_translations(lines: &mut [LyricLine], enhanced: &mut [EnhancedLine]) {
    let timed = enhanced.iter().any(|line| line.has_word_timing);
    let plain_layout = !timed && is_shifted_plain_bilingual_block(lines);
    let mut aligned = false;
    for index in 1..lines.len() {
        let previous = index - 1;
        if lines[index].text.trim().is_empty() || enhanced[index].has_word_timing {
            continue;
        }
        let original_kind = text_kind(&lines[previous].text);
        let bilingual_pair =
            original_kind.is_some_and(|kind| text_kind(&lines[index].text) == Some(!kind));
        // 无逐字时间并不必然是译文：混合时间精度的原文和合唱行也可能逐行计时。
        // 逐字文档须存在源接口的零值逐字占位；普通文档须整体满足延后译文排列。
        // 不凭语言不同就改动无逐字标签的另一声部。
        if !(if timed {
            enhanced[previous].has_word_timing && enhanced[index].is_timing_placeholder
        } else {
            plain_layout && bilingual_pair
        }) {
            continue;
        }
        let time = time_to_seconds(&enhanced[index].time);
        let previous_time = time_to_seconds(&enhanced[previous].time);
        if time == previous_time && timed {
            // 同刻且有零值占位时可识别已对齐译文，同时保留同刻的真实逐字声部。
            enhanced[index].is_translation = bilingual_pair;
            continue;
        }
        if time <= previous_time {
            continue;
        }
        let boundary = if let Some(next) = enhanced.get(index + 1) {
            time == time_to_seconds(&next.time)
                && (lines[index + 1].text.trim().is_empty()
                    || if timed {
                        next.has_word_timing
                    } else {
                        text_kind(&lines[index + 1].text) == original_kind
                    })
        } else {
            // 已识别出该源的译文排列后，最后一句没有下一原文，仍与上一原文配对。
            aligned && (enhanced[index].is_timing_placeholder || bilingual_pair)
        };
        if !boundary {
            continue;
        }
        lines[index].time = lines[previous].time.clone();
        enhanced[index].time = enhanced[previous].time.clone();
        enhanced[index].line_start_ms = enhanced[previous].line_start_ms;
        enhanced[index].is_translation = true;
        for word in &mut enhanced[index].words {
            word.start_ms = enhanced[index].line_start_ms;
        }
        aligned = true;
    }
}

/// 普通 LRC 没有逐字结构可用于区分原文和译文，因此须完整匹配：
/// 原文/译文成对、脚本交替、译文时间等于下一原文时间，并有末尾空行作为边界。
/// 有前奏说明、已对齐双语或零散对唱时不凭局部重复时间推测行间关系。
fn is_shifted_plain_bilingual_block(lines: &[LyricLine]) -> bool {
    if lines.len() < 5 || lines.len() % 2 == 0 || !lines.last().unwrap().text.trim().is_empty() {
        return false;
    }
    let Some(original_kind) = text_kind(&lines[0].text) else {
        return false;
    };
    lines[..lines.len() - 1]
        .chunks_exact(2)
        .enumerate()
        .all(|(index, pair)| {
            text_kind(&pair[0].text) == Some(original_kind)
                && text_kind(&pair[1].text) == Some(!original_kind)
                && time_to_seconds(&pair[0].time) < time_to_seconds(&pair[1].time)
                && time_to_seconds(&pair[1].time) == time_to_seconds(&lines[(index + 1) * 2].time)
        })
}

/// 无逐字结构时的保守辅助判断：只区分单一的汉字/拉丁字母文本。
/// 混合脚本和无法判别的行不参与普通双语行的时间修正。
fn text_kind(text: &str) -> Option<bool> {
    let han = text.chars().any(|ch| matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{20000}'..='\u{2fa1f}'));
    let latin = text.chars().any(|ch| ch.is_ascii_alphabetic());
    match (han, latin) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

/// 构建普通 LRC 文本。
pub(crate) fn build_lrc_text(tags: &[String], lyric_lines: &[LyricLine]) -> String {
    let mut out_lines = tags.to_vec();
    for item in lyric_lines {
        out_lines.push(format!("[{}]{}", item.time, item.text));
    }
    out_lines.join("\n")
}

/// 构建增强 LRC（逐字时间）文本。
pub(crate) fn build_enhanced_lrc_text(tags: &[String], enhanced_lines: &[EnhancedLine]) -> String {
    let mut out_lines = tags.to_vec();
    for line in enhanced_lines {
        let start = format_timestamp(line.line_start_ms);
        let mut rendered = format!("[{}]", start);
        if !line.has_word_timing {
            // 译文和结束空行保留行级标签，不凭空生成逐字时间或零时长的结束标签。
            rendered.extend(line.words.iter().map(|word| word.text.as_str()));
            out_lines.push(rendered);
            continue;
        }
        for (i, word) in line.words.iter().enumerate() {
            let word_start = format_timestamp(word.start_ms);
            rendered.push_str(&format!("<{}>{}", word_start, word.text));
            if i == line.words.len() - 1 {
                let end_ms = word.start_ms + word.duration_ms;
                rendered.push_str(&format!("<{}>", format_timestamp(end_ms)));
            }
        }
        out_lines.push(rendered);
    }
    out_lines.join("\n")
}

/// 解析完整歌词文本，提取标签、普通行、增强行。
///
/// # 参数
/// - `lrc`: 解密后的歌词文本。
///
/// # 返回
/// 元组 `(tags, lyric_lines, enhanced_lines)`。
pub(crate) fn parse_lyric(lrc: &str) -> (Vec<String>, Vec<LyricLine>, Vec<EnhancedLine>) {
    let mut offset = 1i64;
    let mut offset2 = 1i64;
    let tags = parse_tags(lrc, &mut offset, &mut offset2);
    let (lyric_lines, enhanced_lines) = parse_lyric_lines(lrc, offset, offset2);
    (tags, lyric_lines, enhanced_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 酷我的译文带零值逐字标签，行时间却位于下一句原文的开始处。
    const SHIFTED: &str = "[00:00.000]<500,-500>First <1000,0>line\n\
        [00:01.000]<0,0>第一行\n\
        [00:01.000]<500,-500>Second <1000,0>line\n\
        [00:02.000]<0,0>第二行\n\
        [00:02.000]";

    #[test]
    fn translations_follow_the_original_line_in_both_lyric_formats() {
        let (tags, lines, enhanced) = parse_lyric(SHIFTED);
        assert_eq!(build_lrc_text(&tags, &lines),
            "[00:00.000]First line\n[00:00.000]第一行\n[00:01.000]Second line\n[00:01.000]第二行\n[00:02.000]");
        assert_eq!(build_enhanced_lrc_text(&tags, &enhanced),
            "[00:00.000]<00:00.000>First <00:00.500>line<00:01.000>\n[00:00.000]第一行\n[00:01.000]<00:01.000>Second <00:01.500>line<00:02.000>\n[00:01.000]第二行\n[00:02.000]");
    }

    #[test]
    fn zero_timing_translation_placeholders_do_not_add_blank_lines() {
        let raw = "[00:00.000]<500,-500>First line\n[00:01.000]<0,0> <0,0> \n[00:01.000]<500,-500>Second line\n[00:02.000]";
        let (tags, lines, enhanced) = parse_lyric(raw);
        assert_eq!(
            build_lrc_text(&tags, &lines),
            "[00:00.000]First line\n[00:01.000]Second line\n[00:02.000]"
        );
        assert_eq!(build_enhanced_lrc_text(&tags, &enhanced).lines().count(), 3);
    }

    #[test]
    fn already_aligned_translation_and_simultaneous_timed_lines_keep_their_times() {
        let raw = "[00:00.000]<500,-500>First line\n[00:00.000]<0,0>第一行\n[00:01.000]<500,-500>Second line\n[00:01.000]<0,0>第二行";
        let (_, lines, _) = parse_lyric(raw);
        assert_eq!(
            lines
                .iter()
                .map(|line| line.time.as_str())
                .collect::<Vec<_>>(),
            ["00:00.000", "00:00.000", "00:01.000", "00:01.000"]
        );
        let (_, lines, _) = parse_lyric(
            "[00:00.000]<500,-500>独唱\n[00:01.000]<500,-500>合唱甲\n[00:01.000]<500,-500>合唱乙",
        );
        assert_eq!(
            lines
                .iter()
                .map(|line| line.time.as_str())
                .collect::<Vec<_>>(),
            ["00:00.000", "00:01.000", "00:01.000"]
        );
    }

    #[test]
    fn final_translation_uses_the_final_original_even_without_a_closing_blank() {
        let raw = SHIFTED.strip_suffix("\n[00:02.000]").unwrap();
        let (tags, lines, _) = parse_lyric(raw);
        assert!(build_lrc_text(&tags, &lines).ends_with("[00:01.000]第二行"));
    }

    #[test]
    fn plain_bilingual_end_timestamps_are_aligned_without_reversing_row_order() {
        let (tags, lines, _) = parse_lyric("[00:00.000]First line\n[00:01.000]第一行\n[00:01.000]Second line\n[00:02.000]第二行\n[00:02.000]");
        assert_eq!(build_lrc_text(&tags, &lines),
            "[00:00.000]First line\n[00:00.000]第一行\n[00:01.000]Second line\n[00:01.000]第二行\n[00:02.000]");
        let (_, aligned, _) = parse_lyric(
            "[00:00.000]First line\n[00:00.000]第一行\n[00:01.000]Second line\n[00:01.000]第二行",
        );
        assert_eq!(
            aligned
                .iter()
                .map(|line| line.time.as_str())
                .collect::<Vec<_>>(),
            ["00:00.000", "00:00.000", "00:01.000", "00:01.000"]
        );
    }

    #[test]
    fn monolingual_plain_rows_and_end_markers_do_not_gain_word_timestamps() {
        for raw in [
            "[00:00.000]第一句\n[00:01.000]第二句\n[00:02.000]",
            "[00:00.000]First line\n[00:01.000]Second line\n[00:02.000]",
        ] {
            let (tags, _, enhanced) = parse_lyric(raw);
            assert_eq!(build_enhanced_lrc_text(&tags, &enhanced), raw);
        }
    }

    #[test]
    fn optional_third_word_field_is_not_rendered_as_lyric_text() {
        let (tags, lines, enhanced) = parse_lyric("[00:00.000]<500,-500,0>First <1000,0,0>line");
        assert_eq!(build_lrc_text(&tags, &lines), "[00:00.000]First line");
        assert_eq!(
            build_enhanced_lrc_text(&tags, &enhanced),
            "[00:00.000]<00:00.000>First <00:00.500>line<00:01.000>"
        );
    }

    #[test]
    fn untimed_originals_in_mixed_timing_and_duet_lines_keep_their_own_start() {
        for raw in [
            "[00:00.000]<500,-500>First voice\n[00:01.000]Second voice\n[00:01.000]<500,-500>Third voice",
            "[00:00.000]<500,-500>第一声部\n[00:01.000]第二声部\n[00:01.000]<500,-500>第三声部",
            "[00:00.000]<500,-500>第一声部\n[00:01.000]Second voice\n[00:01.000]<500,-500>第三声部",
            "[00:00.000]<500,-500>First voice\n[00:01.000]第二声部\n[00:01.000]<500,-500>Third voice",
        ] {
            let (_, lines, enhanced) = parse_lyric(raw);
            assert_eq!(lines.iter().map(|line| line.time.as_str()).collect::<Vec<_>>(),
                ["00:00.000", "00:01.000", "00:01.000"]);
            assert_eq!(enhanced[1].line_start_ms, 1000);
        }
    }

    #[test]
    fn zero_timing_end_and_instrumental_break_markers_are_preserved() {
        let (tags, lines, enhanced) = parse_lyric(
            "[00:00.000]<500,-500>First line\n[00:05.000]<0,0>\n[00:10.000]<500,-500>Second line\n[00:15.000]<0,0>",
        );
        assert_eq!(
            build_lrc_text(&tags, &lines),
            "[00:00.000]First line\n[00:05.000]\n[00:10.000]Second line\n[00:15.000]"
        );
        assert_eq!(build_enhanced_lrc_text(&tags, &enhanced),
            "[00:00.000]<00:00.000>First line<00:00.500>\n[00:05.000]\n[00:10.000]<00:10.000>Second line<00:10.500>\n[00:15.000]");
    }

    #[test]
    fn encoded_closing_blank_and_metadata_survive_translation_alignment() {
        let raw = format!("[ti:Synthetic]\n[offset:100]\n{SHIFTED}<0,0>");
        let (tags, lines, enhanced) = parse_lyric(&raw);
        assert_eq!(tags, ["[ti:Synthetic]", "[offset:100]"]);
        assert_eq!(lines.last().unwrap().time, "00:02.000");
        assert_eq!(lines.last().unwrap().text, "");
        assert_eq!(lines[3].time, "00:01.000");
        assert!(build_enhanced_lrc_text(&tags, &enhanced).ends_with("\n[00:02.000]"));
    }

    #[test]
    fn aligned_plain_translation_after_a_different_script_preamble_is_unchanged() {
        let raw = "[00:00.000]Intro credits\n[00:10.000]第一行\n[00:10.000]First line\n[00:20.000]第二行\n[00:20.000]Second line\n[00:30.000]";
        let (tags, lines, enhanced) = parse_lyric(raw);
        assert_eq!(build_lrc_text(&tags, &lines), raw);
        assert_eq!(build_enhanced_lrc_text(&tags, &enhanced), raw);
    }
}

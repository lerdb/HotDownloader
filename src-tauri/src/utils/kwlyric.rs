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
    #[allow(dead_code)]
    pub time: String,
    pub line_start_ms: i64,
    pub words: Vec<Word>,
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
    let word_tag = Regex::new(r"<-?\d+,-?\d+>").unwrap();

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
        let normalized_time = if Regex::new(r"\.\d\d$").unwrap().is_match(&time_str) {
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
        lyric_lines.push(LyricLine {
            time: normalized_time.clone(),
            text: plain_text,
        });

        let line_abs_start = words.first().map(|w| w.start_ms).unwrap_or(line_start_ms);
        enhanced_lines.push(EnhancedLine {
            time: normalized_time.clone(),
            line_start_ms: line_abs_start,
            words,
        });
    }

    (lyric_lines, enhanced_lines)
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

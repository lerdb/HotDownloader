use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// 桌面端和 Web 歌词查询接口的响应。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricData {
    /// 普通 LRC 歌词（合并了逐字时间）
    pub lrc: Option<String>,
    /// 增强型 LRC 歌词（逐字时间）。
    pub elrc: Option<String>,
    /// 解密后的原始 `LyricContent`（QRC 或纯 LRC 字符串）。
    pub raw: Option<String>,
    /// 歌曲是否为纯音乐（简单检测）。
    pub instrumental: bool,
}

static LINE_TIME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[([0-9]+):([0-5][0-9])(?:[.:]([0-9]{1,3}))?\]").unwrap());
static WORD_TIME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<[0-9]+:[0-5][0-9](?:[.:][0-9]{1,3})?>").unwrap());

#[derive(Clone, Copy)]
pub(crate) enum LyricAnchor {
    /// 行号按原文时间标签计数，包含用于清除高亮的空行。
    Line(usize),
    Time(u64),
}

pub(crate) struct AuxiliaryLine {
    pub anchor: LyricAnchor,
    pub text: String,
}

#[derive(Default)]
pub(crate) struct LyricTracks {
    pub original: LyricData,
    pub translation: Vec<AuxiliaryLine>,
    pub romanization: Vec<AuxiliaryLine>,
}

impl From<LyricData> for LyricTracks {
    fn from(original: LyricData) -> Self {
        Self {
            original,
            ..Self::default()
        }
    }
}

impl LyricTracks {
    pub fn render(&self, enhanced: bool, translation: bool, romanization: bool) -> Option<String> {
        let lrc = self
            .original
            .lrc
            .as_deref()
            .filter(|s| !s.trim().is_empty());
        let elrc = self
            .original
            .elrc
            .as_deref()
            .filter(|s| WORD_TIME.is_match(s));
        let plain_fallback;
        let original = match (enhanced, lrc, elrc) {
            (true, _, Some(elrc)) => elrc,
            (_, Some(lrc), _) => lrc,
            (_, None, Some(elrc)) => {
                plain_fallback = WORD_TIME.replace_all(elrc, "");
                plain_fallback.as_ref()
            }
            _ => return None,
        };
        let translation = translation.then_some(&self.translation[..]).unwrap_or(&[]);
        let romanization = romanization
            .then_some(&self.romanization[..])
            .unwrap_or(&[]);
        if self.original.instrumental || (translation.is_empty() && romanization.is_empty()) {
            return Some(original.to_owned());
        }

        let lines = timed_lines(original);
        // 网易云的逐字与逐行版本可能使用不同的行起点。用逐行原文作为译文的参照。
        let reference = lrc.filter(|&lrc| lrc != original).map(timed_lines);
        let translation = align(&lines, reference.as_deref(), translation);
        let romanization = align(&lines, reference.as_deref(), romanization);
        if translation.iter().chain(&romanization).all(Option::is_none) {
            return Some(original.to_owned());
        }

        let mut output = Vec::new();
        let mut timed = 0;
        for (index, raw) in original.lines().enumerate() {
            let start = timed;
            while timed < lines.len() && lines[timed].source_index == index {
                timed += 1;
            }
            if (start..timed).all(|i| translation[i].is_none() && romanization[i].is_none()) {
                output.push(raw.to_owned());
                continue;
            }
            // 有多个时间标签时按各次出现分别输出，避免副歌译文串到另一次。
            for i in start..timed {
                let line = &lines[i];
                let (translated, romanized) = (translation[i], romanization[i]);
                output.push(format!("{}{}", line.tag, line.text));
                for text in [
                    translated,
                    romanized.filter(|text| Some(*text) != translated),
                ]
                .into_iter()
                .flatten()
                {
                    output.push(format!("{}{text}", line.tag));
                }
            }
        }
        let mut output = output.join("\n");
        if original.ends_with('\n') {
            output.push('\n');
        }
        Some(output)
    }
}

pub(crate) fn auxiliary_lrc(text: &str) -> Vec<AuxiliaryLine> {
    let mut lines = timed_lines(text);
    lines.sort_by_key(|line| line.time);
    lines
        .into_iter()
        .filter_map(|line| {
            let text = WORD_TIME.replace_all(line.text, "");
            let text = text.trim();
            (!text.is_empty()).then(|| AuxiliaryLine {
                anchor: LyricAnchor::Time(line.time),
                text: text.to_owned(),
            })
        })
        .collect()
}

pub(crate) fn format_lyric_time(ms: u64) -> String {
    format!("{:02}:{:02}.{:03}", ms / 60000, (ms / 1000) % 60, ms % 1000)
}

struct TimedLine<'a> {
    source_index: usize,
    tag: &'a str,
    time: u64,
    text: &'a str,
    comparable: String,
}

fn timed_lines(text: &str) -> Vec<TimedLine<'_>> {
    let mut lines = Vec::new();
    for (source_index, raw) in text.lines().enumerate() {
        let mut remaining = raw.trim_start_matches('\u{feff}').trim_start();
        let start = lines.len();
        while let Some(captures) = LINE_TIME.captures(remaining) {
            let tag = captures.get(0).unwrap().as_str();
            let time = captures[1].parse::<u64>().ok().and_then(|minutes| {
                let seconds = captures[2].parse::<u64>().ok()?;
                let millis = captures.get(3).map_or(0, |fraction| {
                    let fraction = fraction.as_str();
                    fraction.parse::<u64>().unwrap() * 10_u64.pow(3 - fraction.len() as u32)
                });
                minutes
                    .checked_mul(60000)?
                    .checked_add(seconds * 1000 + millis)
            });
            if let Some(time) = time {
                lines.push(TimedLine {
                    source_index,
                    tag,
                    time,
                    text: "",
                    comparable: String::new(),
                });
            }
            remaining = &remaining[tag.len()..];
        }
        if start < lines.len() {
            let comparable: String = WORD_TIME
                .replace_all(remaining, "")
                .split_whitespace()
                .collect();
            for line in &mut lines[start..] {
                line.text = remaining;
                line.comparable.clone_from(&comparable);
            }
        }
    }
    lines
}

fn unique_index(
    lines: &[TimedLine<'_>],
    predicate: impl Fn(&TimedLine<'_>) -> bool,
) -> Option<usize> {
    let mut candidates = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.comparable.is_empty() && predicate(line));
    let (index, _) = candidates.next()?;
    candidates.next().is_none().then_some(index)
}

fn match_line(
    lines: &[TimedLine<'_>],
    reference: Option<&[TimedLine<'_>]>,
    anchor: LyricAnchor,
) -> Option<(usize, bool)> {
    let time = match anchor {
        LyricAnchor::Line(index) => return lines.get(index).map(|_| (index, true)),
        LyricAnchor::Time(time) => time,
    };
    let bridged = reference.and_then(|reference| {
        let index = unique_index(reference, |line| line.time == time)
            .or_else(|| unique_index(reference, |line| line.time.abs_diff(time) <= 100))?;
        let line = &reference[index];
        let same_text = |candidate: &TimedLine<'_>| candidate.comparable == line.comparable;
        unique_index(reference, same_text)?;
        unique_index(lines, same_text).map(|index| (index, line.time == time))
    });
    // 精确匹配逐行参照时，即使逐字版本的另一句碰巧也在这个时间，仍按对应的原文匹配。
    bridged
        .filter(|&(_, exact)| exact)
        .or_else(|| unique_index(lines, |line| line.time == time).map(|index| (index, true)))
        .or(bridged)
        .or_else(|| {
            unique_index(lines, |line| line.time.abs_diff(time) <= 100).map(|index| (index, false))
        })
}

fn align<'a>(
    lines: &[TimedLine<'_>],
    reference: Option<&[TimedLine<'_>]>,
    auxiliary: &'a [AuxiliaryLine],
) -> Vec<Option<&'a str>> {
    let mut aligned = vec![None; lines.len()];
    let matches: Vec<_> = auxiliary
        .iter()
        .filter_map(|line| {
            let (index, exact) = match_line(lines, reference, line.anchor)?;
            let text = line.text.trim();
            let original = &lines[index].comparable;
            if text.is_empty()
                || original.is_empty()
                || text.split_whitespace().collect::<String>() == *original
            {
                return None;
            }
            Some((index, exact, text, line.anchor))
        })
        .collect();
    let mut exact = vec![false; lines.len()];
    for &(index, is_exact, _, _) in &matches {
        exact[index] |= is_exact;
    }

    let mut ambiguous = vec![false; lines.len()];
    for &(index, is_exact, text, _) in &matches {
        if exact[index] != is_exact {
            continue;
        }
        if let Some(previous) = aligned[index].replace(text) {
            ambiguous[index] |= previous != text;
        }
    }
    aligned.fill(None);
    let mut last = None;
    for (index, is_exact, text, anchor) in matches {
        if exact[index] != is_exact || ambiguous[index] {
            continue;
        }
        if matches!(anchor, LyricAnchor::Time(_)) {
            let position = (lines[index].time, index);
            if last.is_some_and(|previous| previous >= position) {
                continue;
            }
            last = Some(position);
        }
        aligned[index] = Some(text);
    }
    aligned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyric_alignment_prefers_exact_times_and_keeps_repeated_lines_distinct() {
        let tracks = LyricTracks {
            original: LyricData {
                lrc: Some("[00:01.00][00:07.00]Repeat\n[00:03.00]Next\n[00:03.080]Nearby".into()),
                ..Default::default()
            },
            translation: auxiliary_lrc(
                "[00:00.950]Fuzzy\n[00:01.000]First\n[00:03.040]Ambiguous\n[00:07.0]Last",
            ),
            ..Default::default()
        };
        assert_eq!(
            tracks.render(false, true, false).as_deref(),
            Some("[00:01.00]Repeat\n[00:01.00]First\n[00:07.00]Repeat\n[00:07.00]Last\n[00:03.00]Next\n[00:03.080]Nearby")
        );
    }
}

import type { LyricResponse } from '../types'

export interface LyricWord {
    text: string
    startTime: number
    endTime: number
}

export interface LyricLine {
    time: number
    text: string
    translation?: string
    words?: LyricWord[]
}

export interface ParsedLyrics {
    lines: LyricLine[]
    synced: boolean
    hasWordTiming: boolean
}

const LINE_TIME_TAG_RE = /\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\]/g
const WORD_TIME_TAG_RE = /<(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?>/g
const OFFSET_TAG_RE = /^\[offset:\s*([+-]?\d+)\s*\]$/i
const METADATA_TAG_RE = /^\[(ti|ar|al|by|re|ve|kana|length|total):.*\]$/i

function toSeconds(minStr: string, secStr: string, fracStr?: string): number {
    const minutes = Number(minStr)
    const seconds = Number(secStr)
    let frac = 0
    if (fracStr) {
        if (fracStr.length === 1) frac = Number(fracStr) / 10
        else if (fracStr.length === 2) frac = Number(fracStr) / 100
        else frac = Number(fracStr.slice(0, 3)) / 1000
    }
    return Math.max(0, minutes * 60 + seconds + frac)
}

function parseEnhancedWords(
    rawText: string,
    lineStart: number,
    offsetSeconds: number,
): { text: string; words?: LyricWord[] } {
    WORD_TIME_TAG_RE.lastIndex = 0
    const matches = [...rawText.matchAll(WORD_TIME_TAG_RE)]
    if (!matches.length) {
        return { text: rawText.trim() }
    }

    const words: LyricWord[] = []
    let plain = ''

    for (let i = 0; i < matches.length; i++) {
        const match = matches[i]
        const start = Math.max(
            0,
            toSeconds(match[1], match[2], match[3]) - offsetSeconds,
        )
        const textStart = (match.index ?? 0) + match[0].length
        const textEnd =
            i + 1 < matches.length
                ? (matches[i + 1].index ?? rawText.length)
                : rawText.length
        const segment = rawText.slice(textStart, textEnd)

        if (!segment) {
            // QQ 音乐 / 酷我末尾常带显式结束时间标签 `<end>`，用于闭合最后一个字的时间。
            if (words.length > 0) {
                words[words.length - 1].endTime = Math.max(
                    words[words.length - 1].startTime,
                    start,
                )
            }
            continue
        }

        const nextStart =
            i + 1 < matches.length
                ? Math.max(
                      start,
                      toSeconds(
                          matches[i + 1][1],
                          matches[i + 1][2],
                          matches[i + 1][3],
                      ) - offsetSeconds,
                  )
                : start + 0.6

        words.push({
            text: segment,
            startTime: Math.max(lineStart, start),
            endTime: nextStart,
        })
        plain += segment
    }

    const trimmed = plain.trim()
    if (!words.length || !trimmed) {
        return { text: rawText.replace(WORD_TIME_TAG_RE, '').trim() }
    }

    return {
        text: trimmed,
        words,
    }
}

function parseLrcText(raw: string): ParsedLyrics {
    const rawLines = raw.split(/\r?\n/)
    let offsetSeconds = 0

    for (const line of rawLines) {
        const offsetMatch = line.trim().match(OFFSET_TAG_RE)
        if (offsetMatch) {
            offsetSeconds = Number(offsetMatch[1]) / 1000
            break
        }
    }

    const timedEntries: LyricLine[] = []
    const plainLines: string[] = []
    let hasWordTiming = false

    for (const rawLine of rawLines) {
        const trimmed = rawLine.trim()
        if (!trimmed) continue
        if (OFFSET_TAG_RE.test(trimmed) || METADATA_TAG_RE.test(trimmed)) {
            continue
        }

        LINE_TIME_TAG_RE.lastIndex = 0
        const timeMatches = [...trimmed.matchAll(LINE_TIME_TAG_RE)]
        if (!timeMatches.length) {
            // 跳过仅含未知方括号元信息的行
            if (!/^\[[^\]]+\]$/.test(trimmed)) {
                plainLines.push(trimmed)
            }
            continue
        }

        const contentRaw = trimmed.replace(LINE_TIME_TAG_RE, '')
        for (const match of timeMatches) {
            const time = Math.max(
                0,
                toSeconds(match[1], match[2], match[3]) - offsetSeconds,
            )
            const parsed = parseEnhancedWords(contentRaw, time, offsetSeconds)
            if (!parsed.text) continue
            if (parsed.words?.length) {
                hasWordTiming = true
            }
            timedEntries.push({
                time,
                text: parsed.text,
                words: parsed.words,
            })
        }
    }

    if (timedEntries.length > 0) {
        timedEntries.sort((a, b) => a.time - b.time)

        // 合并同时间戳的双语/翻译行（如网易云 / QQ 音乐的译文行）
        const merged: LyricLine[] = []
        for (const entry of timedEntries) {
            const prev = merged[merged.length - 1]
            if (
                prev &&
                Math.abs(prev.time - entry.time) <= 0.03 &&
                !prev.translation &&
                entry.text !== prev.text &&
                !entry.words?.length
            ) {
                prev.translation = entry.text
            } else {
                merged.push(entry)
            }
        }

        // 用下一行起始时间补齐未显式闭合的末字结束时间
        for (let i = 0; i < merged.length; i++) {
            const words = merged[i].words
            if (!words?.length) continue
            const lastWord = words[words.length - 1]
            const nextTime =
                i + 1 < merged.length
                    ? merged[i + 1].time
                    : lastWord.startTime + 4
            if (lastWord.endTime <= lastWord.startTime) {
                lastWord.endTime = Math.max(lastWord.startTime + 0.5, nextTime)
            }
        }

        return {
            lines: merged,
            synced: true,
            hasWordTiming,
        }
    }

    if (plainLines.length > 0) {
        return {
            lines: plainLines.map((text) => ({ time: 0, text })),
            synced: false,
            hasWordTiming: false,
        }
    }

    return {
        lines: [],
        synced: false,
        hasWordTiming: false,
    }
}

/** 优先使用带逐字时间的 elrc，若无再回退到标准 lrc；并尽量补充翻译行。 */
export function parseLyricResponse(response: LyricResponse): ParsedLyrics {
    const elrcParsed = response.elrc ? parseLrcText(response.elrc) : null
    const lrcParsed = response.lrc ? parseLrcText(response.lrc) : null

    if (elrcParsed && elrcParsed.lines.length > 0) {
        // 如果 lrc 中包含同时间戳译文而 elrc 没有，则补充到对应行
        if (lrcParsed && lrcParsed.synced) {
            for (const line of elrcParsed.lines) {
                if (line.translation) continue
                const match = lrcParsed.lines.find(
                    (l) =>
                        Math.abs(l.time - line.time) <= 0.05 &&
                        l.translation,
                )
                if (match?.translation) {
                    line.translation = match.translation
                }
            }
        }
        return elrcParsed
    }

    if (lrcParsed && lrcParsed.lines.length > 0) {
        return lrcParsed
    }

    return {
        lines: [],
        synced: false,
        hasWordTiming: false,
    }
}

/** 根据当前播放时间查找高亮歌词索引（含轻微提前量让切换更跟拍）。 */
export function findActiveLyricIndex(
    lines: LyricLine[],
    currentTime: number,
): number {
    if (!lines.length) return -1
    const probe = currentTime + 0.12
    let low = 0
    let high = lines.length - 1
    let active = -1

    while (low <= high) {
        const mid = (low + high) >> 1
        if (lines[mid].time <= probe) {
            active = mid
            low = mid + 1
        } else {
            high = mid - 1
        }
    }

    return active
}

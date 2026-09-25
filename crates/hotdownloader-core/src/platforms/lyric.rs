/// 通用歌词数据结构，所有平台实现都返回此结构。
/// 目前与 QQ 音乐的 LyricResponse 字段一致，未来其他平台可复用。
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricData {
    /// 普通 LRC 歌词（合并了逐字时间）
    pub lrc: Option<String>,
    /// 增强型 LRC 歌词（逐字时间），仅当 QRC 可用时提供。
    pub elrc: Option<String>,
    /// 解密后的原始 `LyricContent`（QRC 或纯 LRC 字符串）。
    pub raw: Option<String>,
    /// 歌曲是否为纯音乐（简单检测）。
    pub instrumental: bool,
}

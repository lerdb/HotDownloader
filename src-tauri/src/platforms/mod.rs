use serde::{Deserialize, Serialize};

/// 支持的音乐平台
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    /// QQ 音乐
    #[serde(rename = "qqmusic")]
    QqMusic,
}

impl Platform {
    /// 从字符串解析平台标识（前端传入）
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "qqmusic" => Ok(Platform::QqMusic),
            _ => Err(format!("不支持的平台: {}", s)),
        }
    }
}

pub mod lyric;

/// QQ 音乐平台实现模块
pub mod qqmusic;

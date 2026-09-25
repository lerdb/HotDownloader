use serde::{Deserialize, Serialize};

/// 支持的音乐平台标识；具体 API 实现由运行时的 worker 提供。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    /// QQ 音乐
    #[serde(rename = "qqmusic")]
    QqMusic,
    /// 酷我音乐
    #[serde(rename = "kuwo")]
    Kuwo,
}

impl Platform {
    /// 从前端或持久化记录中的平台字符串解析。
    pub fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "qqmusic" => Ok(Self::QqMusic),
            "kuwo" => Ok(Self::Kuwo),
            _ => Err(format!("不支持的平台: {value}")),
        }
    }
}

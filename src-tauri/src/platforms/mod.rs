// 平台标识与查询解析位于共享核心；登录存储和下载事件仍由 Tauri 适配。
pub use hotdownloader_core::platform::Platform;

pub use hotdownloader_core::platforms::lyric;

/// QQ 音乐平台实现模块
pub mod qqmusic;

/// 酷我音乐平台实现模块
pub mod kuwo;

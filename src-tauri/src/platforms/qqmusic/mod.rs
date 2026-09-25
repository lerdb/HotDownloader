pub(crate) mod login;

// 平台查询函数接收普通设置值；Tauri 命令在调用前读取歌手分隔符。
pub(crate) use hotdownloader_core::platforms::qqmusic::{
    album, artist, lyrics, playlist, search, suggest,
};

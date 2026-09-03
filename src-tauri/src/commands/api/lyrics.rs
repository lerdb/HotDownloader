//! 歌词获取命令路由层

use crate::platforms::lyric::LyricData;
use crate::platforms::Platform;
use tauri::command;

/// 内部函数：根据平台获取歌词，返回通用 LyricData。
pub(crate) async fn get_lyric_by_id_inner(
    platform: Platform,
    song_id: u64,
) -> Result<LyricData, String> {
    match platform {
        Platform::QqMusic => crate::platforms::qqmusic::lyrics::get_lyric_by_id(song_id)
            .await
            .map(|resp| LyricData {
                lrc: resp.lrc,
                elrc: resp.elrc,
                raw: resp.raw,
                instrumental: resp.instrumental,
            }),
        Platform::Kuwo => Err("该平台暂不支持歌词获取".into()),
    }
}

/// Tauri 命令：获取歌词。
#[command]
pub async fn get_lyric_by_id(platform: String, song_id: u64) -> Result<LyricData, String> {
    let p = Platform::from_str(&platform)?;
    get_lyric_by_id_inner(p, song_id).await
}

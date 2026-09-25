//! Web 音乐查询入口。只做请求参数校验和平台分派，解析与网络请求复用共享核心。

use hotdownloader_core::platform::Platform;
use hotdownloader_core::platforms;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicRequest {
    platform: String,
    #[serde(default)]
    keyword: String,
    #[serde(default)]
    input: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    song_id: u64,
    #[serde(default = "first_page")]
    page: u32,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn first_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    20
}

pub async fn execute(
    action: &str,
    request: MusicRequest,
    artist_separator: &str,
) -> Result<Value, String> {
    let platform = Platform::from_str(&request.platform)?;
    if request.page == 0 || request.limit == 0 || request.limit > 100 {
        return Err("无效的分页参数".into());
    }
    let result = match (platform, action) {
        (Platform::QqMusic, "songs/search") => {
            platforms::qqmusic::search::search_songs(
                artist_separator,
                request.keyword,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::Kuwo, "songs/search") => {
            platforms::kuwo::search::search_songs(
                artist_separator,
                request.keyword,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::QqMusic, "hot-keywords") => {
            platforms::qqmusic::suggest::fetch_hot_keywords().await?
        }
        (Platform::Kuwo, "hot-keywords") => platforms::kuwo::suggest::fetch_hot_keywords().await?,
        (Platform::QqMusic, "suggestions") => {
            platforms::qqmusic::suggest::fetch_suggestions(request.keyword).await?
        }
        (Platform::Kuwo, "suggestions") => {
            platforms::kuwo::suggest::fetch_suggestions(request.keyword).await?
        }
        (Platform::QqMusic, "playlists/fetch") => {
            platforms::qqmusic::playlist::fetch_playlist_songs(artist_separator, request.input)
                .await?
        }
        (Platform::Kuwo, "playlists/fetch") => {
            platforms::kuwo::playlist::fetch_playlist_songs(artist_separator, request.input).await?
        }
        (Platform::QqMusic, "playlists/search") => {
            platforms::qqmusic::playlist::search_playlists(
                request.keyword,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::Kuwo, "playlists/search") => {
            platforms::kuwo::playlist::search_playlists(
                request.keyword,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::QqMusic, "albums/search") => {
            platforms::qqmusic::search::search_albums(
                artist_separator,
                request.keyword,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::Kuwo, "albums/search") => {
            platforms::kuwo::search::search_albums(
                artist_separator,
                request.keyword,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::QqMusic, "albums/fetch") => {
            platforms::qqmusic::album::fetch_album_songs(artist_separator, request.id).await?
        }
        (Platform::Kuwo, "albums/fetch") => {
            platforms::kuwo::album::fetch_album_songs(artist_separator, request.id).await?
        }
        (Platform::QqMusic, "artists/search") => {
            platforms::qqmusic::search::search_artists(request.keyword, request.page, request.limit)
                .await?
        }
        (Platform::Kuwo, "artists/search") => {
            platforms::kuwo::search::search_artists(request.keyword, request.page, request.limit)
                .await?
        }
        (Platform::QqMusic, "artists/songs") => {
            platforms::qqmusic::artist::fetch_artist_songs(
                artist_separator,
                request.id,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::Kuwo, "artists/songs") => {
            platforms::kuwo::artist::fetch_artist_songs(
                artist_separator,
                request.id,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::QqMusic, "artists/albums") => {
            platforms::qqmusic::artist::fetch_artist_albums(
                artist_separator,
                request.id,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::Kuwo, "artists/albums") => {
            platforms::kuwo::artist::fetch_artist_albums(
                artist_separator,
                request.id,
                request.page,
                request.limit,
            )
            .await?
        }
        (Platform::Kuwo, "cover") => {
            return Ok(Value::String(
                platforms::kuwo::cover::fetch_cover(request.song_id).await?,
            ));
        }
        (Platform::QqMusic, "cover") => {
            return Err("QQ 音乐封面已包含在搜索结果中，无需单独获取".into());
        }
        (Platform::QqMusic, "lyrics") => {
            return serde_json::to_value(
                platforms::qqmusic::lyrics::get_lyric_by_id(request.song_id).await?,
            )
            .map_err(|error| error.to_string());
        }
        (Platform::Kuwo, "lyrics") => {
            return serde_json::to_value(
                platforms::kuwo::lyrics::get_lyric_by_id(request.song_id).await?,
            )
            .map_err(|error| error.to_string());
        }
        _ => return Err("未知音乐操作".into()),
    };
    serde_json::from_str(&result).map_err(|error| format!("解析平台响应失败: {error}"))
}

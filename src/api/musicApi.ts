import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@tauri-apps/api/core'
import type {
    SongInfo,
    SearchResponse,
    SearchSuggestionData,
    PlaylistSongsResponse,
    PlaylistSearchResponse,
    AlbumSearchResponse,
    AlbumSongsResponse,
    ArtistSearchResponse,
    ArtistSongsResponse,
    ArtistAlbumsResponse,
    UpdateInfo,
    LyricResponse
} from '../types'
import { cachedInvoke } from './cachedInvoke'
import { webRequest } from './webClient'

const native = isTauri()

/** 两端使用同一组参数与返回类型；Tauri 的旧命令仍返回 JSON 字符串。 */
async function musicCall<T>(
    command: string,
    action: string,
    args: Record<string, unknown>,
): Promise<T> {
    if (native) {
        return JSON.parse(await invoke<string>(command, args)) as T
    }
    return webRequest<T>(`/api/music/${action}`, {
        method: 'POST',
        body: JSON.stringify(args),
    })
}

export async function searchSongs(
    platform: string,
    keyword: string,
    page: number = 1,
    limit: number = 20
): Promise<SearchResponse> {
    const parsed = await musicCall<SearchResponse>('search_songs', 'songs/search', {
        platform,
        keyword,
        page,
        limit
    })
    if (Array.isArray(parsed)) {
        return {
            songs: (parsed as unknown as SongInfo[]).map(s => ({
                ...s,
                platform
            })),
            has_more: false
        }
    }
    // 为返回的歌曲补充平台信息
    parsed.songs = parsed.songs.map(s => ({
        ...s,
        platform
    }))
    return parsed
}

// 获取热搜关键词
export async function getHotKeywords(platform: string): Promise<string[]> {
    return musicCall('fetch_hot_keywords', 'hot-keywords', { platform })
}

// 获取搜索建议
export async function fetchSuggestions(platform: string, keyword: string): Promise<SearchSuggestionData> {
    return musicCall('fetch_suggestions', 'suggestions', {
        platform,
        keyword
    })
}

// 获取歌单
export async function fetchPlaylistSongs(platform: string, input: string): Promise<PlaylistSongsResponse> {
    const parsed = await musicCall<PlaylistSongsResponse>('fetch_playlist_songs', 'playlists/fetch', {
        platform,
        input
    })
    // 为返回的歌曲补充平台信息
    parsed.songs = parsed.songs.map(s => ({
        ...s,
        platform
    }))
    return parsed
}

// 搜索歌单
export async function searchPlaylists(
    platform: string,
    keyword: string,
    page: number = 1,
    limit: number = 20
): Promise<PlaylistSearchResponse> {
    const parsed = await musicCall<PlaylistSearchResponse>('search_playlists', 'playlists/search', {
        platform,
        keyword,
        page,
        limit
    })
    // 为返回的歌单补充平台信息
    parsed.playlists = parsed.playlists.map(p => ({
        ...p,
        platform
    }))
    return parsed
}

// 检查 GitHub 最新版本
export async function checkForUpdate(): Promise<UpdateInfo> {
    if (!native) {
        throw new Error('网页版本由服务端更新')
    }
    const json = await invoke<string>('check_update')
    return JSON.parse(json) as UpdateInfo
}

/**
 * 根据歌曲 ID 获取歌词
 * @param platform 平台标识
 * @param songId 歌曲数字 ID
 */
export async function getLyricBySongId(platform: string, songId: number): Promise<LyricResponse> {
    if (!native) {
        return musicCall('get_lyric_by_id', 'lyrics', { platform, songId })
    }
    return invoke<LyricResponse>('get_lyric_by_id', {
        platform,
        songId
    });
}

// 请求系统通知权限
export async function requestNotificationPermission(): Promise<boolean> {
    if (!native) {
        return typeof Notification !== 'undefined' &&
            await Notification.requestPermission() === 'granted'
    }
    return invoke<boolean>('request_notification_permission')
}

// 检查系统通知权限是否已授予
export async function checkNotificationPermission(): Promise<boolean> {
    if (!native) {
        return typeof Notification !== 'undefined' && Notification.permission === 'granted'
    }
    return invoke<boolean>('check_notification_permission')
}

/**
 * 按需获取歌曲封面 URL（酷我专用）。
 *
 * 搜索接口不再并发拉封面，改为搜索结果展示时由各 SongItem 组件
 * 单独调用此函数获取。多次调用自动去重并缓存。
 *
 * @param platform 平台标识
 * @param songId 歌曲数字 ID
 * @returns 封面图片 URL
 */
export function fetchCover(platform: string, songId: number): Promise<string> {
    if (!native) {
        return musicCall('fetch_cover', 'cover', { platform, songId })
    }
    return cachedInvoke<string>('fetch_cover', {
        platform,
        songId
    })
}

// ==================== 登录相关 API ====================

// 登录二维码返回
export interface QrLoginResult {
    qrcode_id: string
    qr_base64: string
}

// 登录轮询结果
export interface LoginCheckResult {
    status: 'waiting' | 'scanned' | 'confirmed' | 'expired' | 'canceled' | 'error'
    credentials?: LoginCredentials
    message?: string
}

// 登录凭据
export interface LoginCredentials {
    uin: string
    authst: string
    refreshToken: string
    refreshKey: string
    accessToken: string
    openid: string
}

// 获取登录二维码
export async function createQrLogin(platform: string): Promise<QrLoginResult> {
    if (!native) {
        return webRequest('/api/login/qr', { method: 'POST' })
    }
    const json = await invoke<string>('create_qr_login', { platform })
    return JSON.parse(json) as QrLoginResult
}

// 轮询二维码登录状态
export async function checkQrLogin(platform: string, qrcodeId: string): Promise<LoginCheckResult> {
    if (!native) {
        return webRequest(`/api/login/qr/${encodeURIComponent(qrcodeId)}`)
    }
    const json = await invoke<string>('check_qr_login', {
        platform,
        qrcodeId
    })
    return JSON.parse(json) as LoginCheckResult
}

// 使用 uin + authst 手动登录，可选字段用于刷新登录
export async function loginWithUinAuthst(
    platform: string,
    uin: string,
    authst: string,
    refreshToken: string = '',
    refreshKey: string = '',
    accessToken: string = '',
    openid: string = ''
): Promise<LoginCredentials> {
    if (!native) {
        return webRequest('/api/login/manual', {
            method: 'POST',
            body: JSON.stringify({ uin, authst, refreshToken, refreshKey, accessToken, openid }),
        })
    }
    const json = await invoke<string>('login_with_uin_authst', {
        platform,
        uin,
        authst,
        refreshToken,
        refreshKey,
        accessToken,
        openid,
    })
    return JSON.parse(json) as LoginCredentials
}

// 退出登录
export async function logout(platform: string): Promise<void> {
    if (!native) {
        await webRequest('/api/login/logout', { method: 'POST' })
        return
    }
    await invoke('logout', { platform })
}

// 查询登录状态
export async function getLoginStatus(platform: string): Promise<{ logged_in: boolean; uin: string }> {
    if (!native) {
        return webRequest('/api/login/status')
    }
    const json = await invoke<string>('get_login_status', { platform })
    return JSON.parse(json) as { logged_in: boolean; uin: string }
}

export async function searchAlbums(platform: string, keyword: string, page = 1, limit = 20): Promise<AlbumSearchResponse> {
    return musicCall('search_albums', 'albums/search', {
        platform,
        keyword,
        page,
        limit
    })
}

export async function fetchAlbumSongs(platform: string, id: string): Promise<AlbumSongsResponse> {
    const result = await musicCall<AlbumSongsResponse>('fetch_album_songs', 'albums/fetch', {
        platform,
        id
    })
    result.songs = result.songs.map(song => ({
        ...song,
        platform
    }))
    return result
}

export async function searchArtists(platform: string, keyword: string, page = 1, limit = 20): Promise<ArtistSearchResponse> {
    return musicCall('search_artists', 'artists/search', {
        platform,
        keyword,
        page,
        limit
    })
}

export async function fetchArtistSongs(platform: string, id: string, page = 1, limit = 20): Promise<ArtistSongsResponse> {
    const result = await musicCall<ArtistSongsResponse>('fetch_artist_songs', 'artists/songs', {
        platform,
        id,
        page,
        limit
    })
    result.songs = result.songs.map(song => ({
        ...song,
        platform
    }))
    return result
}

export async function fetchArtistAlbums(platform: string, id: string, page = 1, limit = 20): Promise<ArtistAlbumsResponse> {
    return musicCall('fetch_artist_albums', 'artists/albums', {
        platform,
        id,
        page,
        limit
    })
}

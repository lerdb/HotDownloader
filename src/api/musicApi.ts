import { invoke } from '@tauri-apps/api/core'
import type { SongInfo, SearchResponse, SearchSuggestionData, PlaylistSongsResponse, UpdateInfo, LyricResponse } from '../types'

export async function searchSongs(
    platform: string,
    keyword: string,
    page: number = 1,
    limit: number = 20
): Promise<SearchResponse> {
    const json = await invoke<string>('search_songs', { platform, keyword, page, limit })
    const parsed = JSON.parse(json) as SearchResponse
    if (Array.isArray(parsed)) {
        return { songs: (parsed as unknown as SongInfo[]).map(s => ({ ...s, platform })), has_more: false }
    }
    // 为返回的歌曲补充平台信息
    parsed.songs = parsed.songs.map(s => ({ ...s, platform }))
    return parsed
}

export async function fetchDownloadLink(
    platform: string,
    songMid: string,
    filename: string
): Promise<{ url: string; key: string }> {
    const json = await invoke<string>('fetch_download_link', { platform, songMid, filename })
    return JSON.parse(json) as { url: string; key: string }
}

// 获取热搜关键词
export async function getHotKeywords(platform: string): Promise<string[]> {
    const json = await invoke<string>('fetch_hot_keywords', { platform })
    return JSON.parse(json) as string[]
}

// 获取搜索建议
export async function fetchSuggestions(platform: string, keyword: string): Promise<SearchSuggestionData> {
    const json = await invoke<string>('fetch_suggestions', { platform, keyword })
    return JSON.parse(json) as SearchSuggestionData
}

// 获取歌单
export async function fetchPlaylistSongs(platform: string, input: string): Promise<PlaylistSongsResponse> {
    const json = await invoke<string>('fetch_playlist_songs', { platform, input })
    const parsed = JSON.parse(json) as PlaylistSongsResponse
    // 为返回的歌曲补充平台信息
    parsed.songs = parsed.songs.map(s => ({ ...s, platform }))
    return parsed
}

// 检查 GitHub 最新版本
export async function checkForUpdate(): Promise<UpdateInfo> {
    const json = await invoke<string>('check_update')
    return JSON.parse(json) as UpdateInfo
}

/**
 * 根据歌曲 ID 获取歌词
 * @param platform 平台标识
 * @param songId 歌曲数字 ID
 */
export async function getLyricBySongId(platform: string, songId: number): Promise<LyricResponse> {
    return invoke<LyricResponse>('get_lyric_by_id', { platform, songId });
}

// 检查下载路径是否存在，返回原始路径、是否存在及建议的重命名路径
export async function checkDownloadPath(params: {
    songId: number
    songMid: string
    songTitle: string
    artist: string
    album: string
    coverUrl: string
    qualityFilename: string
    quality: string
}): Promise<{ original_path: string; exists: boolean; suggested_path: string; is_saf: boolean }> {
    const json = await invoke<string>('check_download_path', {
        songId: params.songId,
        songMid: params.songMid,
        songTitle: params.songTitle,
        artist: params.artist,
        album: params.album,
        coverUrl: params.coverUrl,
        qualityFilename: params.qualityFilename,
        quality: params.quality,
    })
    return JSON.parse(json)
}

// 请求系统通知权限
export async function requestNotificationPermission(): Promise<boolean> {
    return invoke<boolean>('request_notification_permission')
}

// 检查系统通知权限是否已授予
export async function checkNotificationPermission(): Promise<boolean> {
    return invoke<boolean>('check_notification_permission')
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
    const json = await invoke<string>('create_qr_login', { platform })
    return JSON.parse(json) as QrLoginResult
}

// 轮询二维码登录状态
export async function checkQrLogin(platform: string, qrcodeId: string): Promise<LoginCheckResult> {
    const json = await invoke<string>('check_qr_login', { platform, qrcodeId })
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
    await invoke('logout', { platform })
}

// 查询登录状态
export async function getLoginStatus(platform: string): Promise<{ logged_in: boolean; uin: string }> {
    const json = await invoke<string>('get_login_status', { platform })
    return JSON.parse(json) as { logged_in: boolean; uin: string }
}

// 所有品质标签，按从低到高排序
export const ALL_QUALITY_ORDER: string[] = [
    '48kaac',
    '96kaac',
    '192kaac',
    '96kogg',
    '100kogg',
    '192kogg',
    '300kogg',
    '128kmp3',
    '320kmp3',
    'ape',
    'flac',
    'hires',
    '臻品全景声',
    '臻品全景声 5.1',
    '臻品全景声 7.1.4',
    '臻品母带',
]

/**
 * 默认降级顺序：从高到低。
 *
 * 该常量只描述应用内置默认值。运行时必须读取设置中的
 * `qualityDowngradeOrder`，否则用户在设置页调整顺序后不会真正生效。
 */
export const QUALITY_DOWNGRADE_ORDER: string[] = [...ALL_QUALITY_ORDER].reverse()

/**
 * 将持久化的降级顺序修复为完整、无重复且只包含已知音质的数组。
 *
 * 设置数据可能来自旧版本（字段不存在），也可能因为手工修改而含有重复项、
 * 未知项或错误类型。标准化时先保留用户有效项的相对顺序，再按默认顺序补入
 * 新版本新增或用户数据中缺失的音质。这样升级应用不会清空用户已有偏好。
 */
export function normalizeQualityDowngradeOrder(value: unknown): string[] {
    const knownQualities = new Set(QUALITY_DOWNGRADE_ORDER)
    const seen = new Set<string>()
    const normalized: string[] = []

    if (Array.isArray(value)) {
        for (const item of value) {
            if (typeof item !== 'string' || !knownQualities.has(item) || seen.has(item)) {
                continue
            }
            seen.add(item)
            normalized.push(item)
        }
    }

    // 缺失项通常来自应用升级后新增的音质；按默认相对顺序追加，保证所有音质
    // 始终都能出现在编辑器中，也能作为后续降级候选。
    for (const quality of QUALITY_DOWNGRADE_ORDER) {
        if (!seen.has(quality)) {
            seen.add(quality)
            normalized.push(quality)
        }
    }

    return normalized
}

export type Quality = string  // 不再限制字面量，兼容所有后端标签

export type TaskStatus = 'waiting' | 'downloading' | 'paused' | 'completed' | 'error' | 'processing'

export interface Settings {
    defaultQuality: Quality
    autoDowngrade: boolean
    /** 用户定义的降级尝试顺序；排在目标音质之后的项目才会被依次尝试。 */
    qualityDowngradeOrder: Quality[]
    downloadDir: string
    namingTemplate: string
    maxConcurrent: number
    jumpToTask: boolean
    // 多名歌手之间的连接字符串
    artistSeparator: string
    // 新增 SAF 文件夹 URI 和名称
    safFolderUri?: string
    safFolderName?: string
    writeMetadata: boolean
    downloadLrc: boolean
    // 登录相关字段，可选，未登录时不设置
    loginUin?: string
    authst?: string
    refreshToken?: string
    refreshKey?: string
    accessToken?: string
    openid?: string
    // 重复文件处理策略：ask=弹窗询问，overwrite=覆盖，rename=自动重命名，cancel=取消下载
    duplicateStrategy?: 'ask' | 'overwrite' | 'rename' | 'cancel'
    // 下载完成后是否发送系统通知
    notifyOnComplete: boolean
}

/** 歌曲可用的单个品质项 */
export interface QualityItem {
    quality: string   // 品质标签，如 "128kmp3", "flac", "臻品母带" 等
    filename: string  // 对应下载文件名
    size: number      // 文件字节大小
}

// 歌曲基本信息
export interface SongInfo {
    platform: string    // 平台标识
    id: number
    mid: string
    title: string
    artist: string
    album: string
    coverUrl: string
    mediaMid: string
    qualities: QualityItem[]
}

// 搜索结果完整返回
export interface SearchResponse {
    songs: SongInfo[]
    has_more: boolean
}

// 歌单基本信息
export interface PlaylistInfo {
    id: string
    name: string
    creator: string
    coverUrl: string
    songCount: number
    playCount: number
}

// 歌单接口完整返回
export interface PlaylistSongsResponse {
    playlist: PlaylistInfo
    songs: SongInfo[]
}

// 歌单搜索结果项
export interface PlaylistSearchItem {
    id: string
    name: string
    creator: string
    coverUrl: string
    songCount: number
    playCount: number
    introduction?: string
    platform?: string
}

// 歌单搜索完整返回
export interface PlaylistSearchResponse {
    playlists: PlaylistSearchItem[]
    has_more: boolean
}

// 搜索建议条目（对应后端 fetch_suggestions 返回的每个 item）
export interface SearchSuggestionItem {
    id?: string
    mid?: string
    name?: string
    singer?: string
    cover?: string | null
    vid?: string          // 仅 MV 类型存在
    [key: string]: unknown
}

// 搜索建议分组数据
export interface SearchSuggestionData {
    song: SearchSuggestionItem[]
    singer: SearchSuggestionItem[]
    album: SearchSuggestionItem[]
    mv: SearchSuggestionItem[]
}

// 歌词响应
export interface LyricResponse {
    lrc: string | null;
    elrc: string | null;
    raw: string | null;
    instrumental: boolean;
}

export interface TaskRecord {
    id: string
    platform: string    // 平台标识
    songId: number
    songMid: string
    songTitle: string
    artist: string
    album: string
    coverUrl: string
    mediaMid: string           // 用于后续可能的操作
    filename: string           // 实际下载的品质文件名
    quality: Quality           // 实际选择的品质标签
    status: TaskStatus
    errorMsg?: string
    filePath?: string
    fileSize: number
    downloaded: number
    retryCount: number
    addedAt: number
    /**
     * 创建任务时歌曲实际提供的全部音质快照。
     *
     * 重试降级不仅要改变品质标签，还必须同步切换后端接口所需的 filename
     * 和预估文件大小；旧任务没有该字段，因此保留为可选以兼容历史数据。
     */
    availableQualities?: QualityItem[]
    speed?: number  // 实时下载速度 (bytes/s)，仅 downloading/paused 状态有意义
    /**
     * 用户在“文件已存在”弹窗中选定的保存路径。
     * - 未设置或为空：使用默认路径（覆盖策略）
     * - 有值：用户选择了“保留两份”，使用建议的重命名路径
     * 持久化后，应用重启重试时复用之，避免落到错误路径。
     */
    savePath?: string
}

export interface DownloadProgressPayload {
    task_id: string
    downloaded: number
    total: number
    speed: number
}

export interface DownloadCompletedPayload {
    task_id: string
    final_path: string
    saf_folder_uri?: string | null
}

export interface DownloadErrorPayload {
    task_id: string
    error_msg: string
}

export interface DownloadLinkExpiredPayload {
    task_id: string
    current_offset: number
}

export const DEFAULT_SETTINGS: Settings = {
    defaultQuality: 'ask',
    autoDowngrade: true,
    // 必须克隆默认数组，避免设置页排序时意外修改全局常量。
    qualityDowngradeOrder: [...QUALITY_DOWNGRADE_ORDER],
    downloadDir: '',
    namingTemplate: '{song} - {artist}',
    maxConcurrent: 3,
    jumpToTask: true,
    artistSeparator: '、',
    writeMetadata: false,
    downloadLrc: false,
    loginUin: '',
    authst: '',
    refreshToken: '',
    refreshKey: '',
    accessToken: '',
    openid: '',
    duplicateStrategy: 'ask',
    notifyOnComplete: false,
}

// GitHub 最新 release 信息
export interface UpdateInfo {
    tag_name: string
    name: string
    body: string
    html_url: string
    published_at: string
    prerelease: boolean
    current_version: string
    assets: UpdateAsset[]
}

// assets 字段：发布资源数组，用于展示下载安装包直链
export interface UpdateAsset {
    name: string
    browser_download_url: string
    size: number
}

// 文件下载完成、处理中事件载荷
export interface DownloadFileCompletePayload {
    task_id: string
}

// 元数据写入失败事件载荷
export interface DownloadMetadataErrorPayload {
    task_id: string
    error_msg: string
}
// 平台配置列表，用于前端平台切换按钮和 API 调用。
// 每个平台包含 key（后端标识）和 label（显示缩写）。
export interface PlatformOption {
    key: string
    label: string
}

export const PLATFORMS: PlatformOption[] = [
    { key: 'qqmusic', label: 'QQ' },
    { key: 'kuwo', label: 'KW' },
]

// 默认平台 key
export const DEFAULT_PLATFORM = PLATFORMS[0].key
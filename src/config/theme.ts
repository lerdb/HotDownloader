import type { GlobalThemeOverrides } from 'naive-ui'

export type ThemeMode = 'system' | 'dark' | 'light'

export type AccentPresetKey =
    | 'simpmusic'
    | 'spotify'
    | 'apple'
    | 'youtube'
    | 'soundcloud'
    | 'ocean'

export interface AccentPreset {
    key: AccentPresetKey
    label: string
    lightPrimary: string
    lightHover: string
    lightPressed: string
    lightSoft: string
    darkPrimary: string
    darkHover: string
    darkPressed: string
    darkSoft: string
}

export const ACCENT_PRESETS: AccentPreset[] = [
    {
        key: 'simpmusic',
        label: 'SimpMusic 青绿',
        lightPrimary: '#128c6e',
        lightHover: '#16a380',
        lightPressed: '#0d6f57',
        lightSoft: 'rgba(18, 140, 110, 0.12)',
        darkPrimary: '#4be3b8',
        darkHover: '#70ecc9',
        darkPressed: '#2cc99d',
        darkSoft: 'rgba(75, 227, 184, 0.16)',
    },
    {
        key: 'spotify',
        label: 'Spotify 绿',
        lightPrimary: '#159643',
        lightHover: '#1aa84d',
        lightPressed: '#107735',
        lightSoft: 'rgba(21, 150, 67, 0.12)',
        darkPrimary: '#1ed760',
        darkHover: '#4ae37f',
        darkPressed: '#17b850',
        darkSoft: 'rgba(30, 215, 96, 0.16)',
    },
    {
        key: 'apple',
        label: 'Apple 绯红',
        lightPrimary: '#d92644',
        lightHover: '#eb3654',
        lightPressed: '#b81d37',
        lightSoft: 'rgba(217, 38, 68, 0.12)',
        darkPrimary: '#ff4f6d',
        darkHover: '#ff738b',
        darkPressed: '#e63352',
        darkSoft: 'rgba(255, 79, 109, 0.16)',
    },
    {
        key: 'youtube',
        label: 'YouTube 烈红',
        lightPrimary: '#cf2033',
        lightHover: '#e52b3f',
        lightPressed: '#ad1727',
        lightSoft: 'rgba(207, 32, 51, 0.12)',
        darkPrimary: '#ff5263',
        darkHover: '#ff7583',
        darkPressed: '#e6394a',
        darkSoft: 'rgba(255, 82, 99, 0.16)',
    },
    {
        key: 'soundcloud',
        label: 'SoundCloud 橙',
        lightPrimary: '#d9480f',
        lightHover: '#f05519',
        lightPressed: '#b83a09',
        lightSoft: 'rgba(217, 72, 15, 0.12)',
        darkPrimary: '#ff7833',
        darkHover: '#ff945c',
        darkPressed: '#e85d17',
        darkSoft: 'rgba(255, 120, 51, 0.16)',
    },
    {
        key: 'ocean',
        label: '深海湛蓝',
        lightPrimary: '#2563eb',
        lightHover: '#3b82f6',
        lightPressed: '#1d4ed8',
        lightSoft: 'rgba(37, 99, 235, 0.12)',
        darkPrimary: '#60a5fa',
        darkHover: '#84b9fb',
        darkPressed: '#3b82f6',
        darkSoft: 'rgba(96, 165, 250, 0.16)',
    },
]

export function getAccentPreset(key: AccentPresetKey): AccentPreset {
    return (
        ACCENT_PRESETS.find((preset) => preset.key === key) ?? ACCENT_PRESETS[0]
    )
}

// 深浅主题共用组件的字号与圆角，避免主题切换时改变布局尺寸。
const common: NonNullable<GlobalThemeOverrides['common']> = {
    fontFamily:
        '"Plus Jakarta Sans", "Inter", "PingFang SC", "Hiragino Sans GB", -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", sans-serif',
    fontSize: '14px',
    borderRadius: '12px',
    borderRadiusSmall: '8px',
}

export function createThemeOverrides(
    isDark: boolean,
    presetKey: AccentPresetKey = 'simpmusic',
): GlobalThemeOverrides {
    const preset = getAccentPreset(presetKey)

    if (isDark) {
        return {
            common: {
                ...common,
                primaryColor: preset.darkPrimary,
                primaryColorHover: preset.darkHover,
                primaryColorPressed: preset.darkPressed,
                primaryColorSuppl: preset.lightPrimary,
                bodyColor: '#0d0f14',
                cardColor: '#161922',
                modalColor: '#1a1e29',
                popoverColor: '#1e222e',
                tableColor: '#161922',
                tableHeaderColor: '#1c202b',
                inputColor: '#1d222d',
                actionColor: '#1d222d',
                hoverColor: 'rgba(255, 255, 255, 0.06)',
                borderColor: 'rgba(255, 255, 255, 0.09)',
                dividerColor: 'rgba(255, 255, 255, 0.08)',
                textColor1: '#f1f5f9',
                textColor2: '#cbd5e1',
                textColor3: '#8b97ab',
            },
            Card: {
                borderRadius: '16px',
                borderColor: 'rgba(255, 255, 255, 0.08)',
            },
            Button: {
                borderRadiusMedium: '10px',
                borderRadiusSmall: '8px',
                borderRadiusTiny: '6px',
                fontWeight: '500',
            },
            Tag: {
                borderRadius: '999px',
            },
            DataTable: {
                borderRadius: '14px',
                thColor: '#1b1f2a',
                tdColor: '#151922',
                tdColorHover: '#1b202c',
                borderColor: 'rgba(255, 255, 255, 0.07)',
            },
            Modal: {
                borderRadius: '20px',
            },
        }
    }

    return {
        common: {
            ...common,
            primaryColor: preset.lightPrimary,
            primaryColorHover: preset.lightHover,
            primaryColorPressed: preset.lightPressed,
            primaryColorSuppl: preset.lightPrimary,
            bodyColor: '#f4f6fa',
            cardColor: '#ffffff',
            modalColor: '#ffffff',
            popoverColor: '#ffffff',
            tableColor: '#ffffff',
            tableHeaderColor: '#f8fafc',
            inputColor: '#ffffff',
            actionColor: '#f1f5f9',
            hoverColor: 'rgba(15, 23, 42, 0.04)',
            borderColor: 'rgba(15, 23, 42, 0.09)',
            dividerColor: 'rgba(15, 23, 42, 0.07)',
            textColor1: '#0f172a',
            textColor2: '#334155',
            textColor3: '#64748b',
        },
        Card: {
            borderRadius: '16px',
            borderColor: 'rgba(15, 23, 42, 0.08)',
        },
        Button: {
            borderRadiusMedium: '10px',
            borderRadiusSmall: '8px',
            borderRadiusTiny: '6px',
            fontWeight: '500',
        },
        Tag: {
            borderRadius: '999px',
        },
        DataTable: {
            borderRadius: '14px',
            thColor: '#f8fafc',
            tdColor: '#ffffff',
            tdColorHover: '#f8fafc',
            borderColor: 'rgba(15, 23, 42, 0.07)',
        },
        Modal: {
            borderRadius: '20px',
        },
    }
}

export const lightThemeOverrides: GlobalThemeOverrides =
    createThemeOverrides(false)

export const darkThemeOverrides: GlobalThemeOverrides =
    createThemeOverrides(true)

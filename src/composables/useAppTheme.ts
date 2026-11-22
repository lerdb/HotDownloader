import { computed, ref, watchEffect } from 'vue'
import { darkTheme, useOsTheme } from 'naive-ui'
import {
    ACCENT_PRESETS,
    createThemeOverrides,
    getAccentPreset,
    type AccentPresetKey,
    type ThemeMode,
} from '../config/theme'

const THEME_MODE_KEY = 'hotdownloader.ui.themeMode'
const ACCENT_PRESET_KEY = 'hotdownloader.ui.accentPreset'

function readStoredMode(): ThemeMode {
    if (typeof window === 'undefined') return 'system'
    try {
        const raw = window.localStorage.getItem(THEME_MODE_KEY)
        if (raw === 'dark' || raw === 'light' || raw === 'system') return raw
    } catch {
        // 忽略受限环境下的存储读取失败
    }
    return 'system'
}

function readStoredAccent(): AccentPresetKey {
    if (typeof window === 'undefined') return 'simpmusic'
    try {
        const raw = window.localStorage.getItem(ACCENT_PRESET_KEY)
        if (ACCENT_PRESETS.some((item) => item.key === raw)) {
            return raw as AccentPresetKey
        }
    } catch {
        // 忽略受限环境下的存储读取失败
    }
    return 'simpmusic'
}

const themeMode = ref<ThemeMode>(readStoredMode())
const accentPreset = ref<AccentPresetKey>(readStoredAccent())

export function setThemeMode(mode: ThemeMode) {
    themeMode.value = mode
    if (typeof window === 'undefined') return
    try {
        window.localStorage.setItem(THEME_MODE_KEY, mode)
    } catch {
        // 忽略存储写入异常
    }
}

export function setAccentPreset(preset: AccentPresetKey) {
    accentPreset.value = preset
    if (typeof window === 'undefined') return
    try {
        window.localStorage.setItem(ACCENT_PRESET_KEY, preset)
    } catch {
        // 忽略存储写入异常
    }
}

/** 跟随系统或用户偏好，统一提供 Naive UI 与全局 CSS 主题变量。 */
export function useAppTheme() {
    // 在 setup 阶段创建一次系统主题监听，由组件作用域管理生命周期。
    const osTheme = useOsTheme()

    const isDark = computed(() => {
        if (themeMode.value === 'dark') return true
        if (themeMode.value === 'light') return false
        return osTheme.value === 'dark'
    })

    const theme = computed(() => (isDark.value ? darkTheme : null))

    const themeOverrides = computed(() =>
        createThemeOverrides(isDark.value, accentPreset.value),
    )

    watchEffect(() => {
        if (typeof document === 'undefined') return
        const root = document.documentElement
        const preset = getAccentPreset(accentPreset.value)
        const dark = isDark.value
        root.dataset.theme = dark ? 'dark' : 'light'
        root.style.colorScheme = dark ? 'dark' : 'light'
        root.style.setProperty(
            '--color-primary',
            dark ? preset.darkPrimary : preset.lightPrimary,
        )
        root.style.setProperty(
            '--color-primary-hover',
            dark ? preset.darkHover : preset.lightHover,
        )
        root.style.setProperty(
            '--color-primary-soft',
            dark ? preset.darkSoft : preset.lightSoft,
        )
    })

    return {
        theme,
        themeOverrides,
        isDark,
        themeMode,
        accentPreset,
        setThemeMode,
        setAccentPreset,
    }
}

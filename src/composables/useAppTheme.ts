import { computed } from 'vue'
import { darkTheme, useOsTheme } from 'naive-ui'
import { darkThemeOverrides, lightThemeOverrides } from '../config/theme'

/** 跟随系统外观，统一提供 Naive UI 的基础主题和组件样式配置。 */
export function useAppTheme() {
    // 在 setup 阶段创建一次系统主题监听，由组件作用域管理生命周期。
    const osTheme = useOsTheme()

    const theme = computed(() => {
        return osTheme.value === 'dark' ? darkTheme : null
    })

    const themeOverrides = computed(() => {
        return osTheme.value === 'dark' ? darkThemeOverrides : lightThemeOverrides
    })

    return {
        theme,
        themeOverrides,
    }
}

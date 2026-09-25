import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import * as settingsApi from '../api/settingsApi'
import type { Settings } from '../types'
import { DEFAULT_SETTINGS, normalizeQualityDowngradeOrder } from '../types'

/**
 * 创建一份彼此独立的默认设置。
 *
 * 对象展开只会浅拷贝；若直接复用 DEFAULT_SETTINGS 中的数组，排序操作会同时
 * 改写默认值，之后“恢复默认”也会得到被污染的顺序。因此数组字段必须单独克隆。
 */
function createDefaultSettings(): Settings {
    return {
        ...DEFAULT_SETTINGS,
        qualityDowngradeOrder: [...DEFAULT_SETTINGS.qualityDowngradeOrder],
    }
}

export const useSettingsStore = defineStore('settings', () => {
    const settings = ref<Settings>(createDefaultSettings())

    async function loadSettings() {
        try {
            const parsed = await settingsApi.loadSettings()
            if (parsed) {
                settings.value = {
                    ...createDefaultSettings(),
                    ...parsed,
                    // 兼容旧版本缺少字段、重复项、未知项以及未来新增音质等情况。
                    qualityDowngradeOrder: normalizeQualityDowngradeOrder(
                        parsed.qualityDowngradeOrder
                    ),
                }
            }
        } catch {
            // 使用默认设置
        }
    }

    async function getDefaultDownloadDir() {
        try {
            settings.value.downloadDir = await settingsApi.getDefaultDownloadDir()
        } catch {
            // 保持现有值
        }
    }

    // 防抖持久化
    let debounceTimer: ReturnType<typeof setTimeout> | null = null
    // 创建和重试任务前需要立即落盘，确保 Rust 读取到当前设置而非防抖前的旧值。
    async function flushSettings() {
        if (debounceTimer) {
            clearTimeout(debounceTimer)
            debounceTimer = null
        }
        await settingsApi.saveSettings(settings.value)
    }
    watch(
        settings,
        () => {
            if (debounceTimer) clearTimeout(debounceTimer)
            debounceTimer = setTimeout(() => {
                flushSettings().catch(console.error)
            }, 500)
        },
        { deep: true }
    )

    // 并发数实时同步到后端
    watch(
        () => settings.value.maxConcurrent,
        (val) => {
            settingsApi.setMaxConcurrent(val).catch(console.error)
        }
    )

    return {
        settings,
        loadSettings,
        getDefaultDownloadDir,
        flushSettings,
    }
})

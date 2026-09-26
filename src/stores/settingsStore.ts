import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import * as settingsApi from '../api/settingsApi'
import type { Settings } from '../types'
import { DEFAULT_SETTINGS, normalizeQualityDowngradeOrder } from '../types'

type SettingKey = keyof Settings

/** 这些字段由下载后端使用；网页版的路径只由服务启动环境决定。 */
const SHARED_KEYS: SettingKey[] = [
    'autoDowngrade',
    'qualityDowngradeOrder',
    'namingTemplate',
    'maxConcurrent',
    'artistSeparator',
    'writeMetadata',
    'downloadLrc',
    'duplicateStrategy',
]
const NATIVE_KEYS: SettingKey[] = [
    ...SHARED_KEYS,
    'defaultQuality',
    'downloadDir',
    'jumpToTask',
    'safFolderUri',
    'safFolderName',
    'notifyOnComplete',
]
const WEB_LOCAL_KEYS: SettingKey[] = ['defaultQuality', 'jumpToTask']
const WEB_LOCAL_PREFIX = 'hotdownloader-local-setting-'

/** 数组必须独立克隆，拖拽排序不能改写全局默认顺序。 */
function createDefaultSettings(): Settings {
    return {
        ...DEFAULT_SETTINGS,
        qualityDowngradeOrder: [...DEFAULT_SETTINGS.qualityDowngradeOrder],
    }
}

function sameValue(left: unknown, right: unknown): boolean {
    return JSON.stringify(left) === JSON.stringify(right)
}

export const useSettingsStore = defineStore('settings', () => {
    const settings = ref<Settings>(createDefaultSettings())
    const conflictFields = ref<SettingKey[]>([])
    const saveError = ref('')
    const native = isTauri()
    const persistedKeys = native ? NATIVE_KEYS : SHARED_KEYS
    const dirty = new Set<SettingKey>()
    const pendingWrites = new Map<SettingKey, unknown>()
    let remote: Partial<Settings> = {}
    let revision = -1
    let loaded = false
    let applyingSnapshot = false
    let debounceTimer: ReturnType<typeof setTimeout> | null = null
    let activeFlush: Promise<void> | null = null

    function effectiveRemote(key: SettingKey): unknown {
        const value = remote[key] ?? DEFAULT_SETTINGS[key]
        return key === 'qualityDowngradeOrder'
            ? normalizeQualityDowngradeOrder(value)
            : value
    }

    function localValue(key: SettingKey): unknown {
        return settings.value[key]
    }

    function setLocalValue(key: SettingKey, value: unknown) {
        const writable = settings.value as unknown as Record<string, unknown>
        writable[key] = value
    }

    function scheduleFlush() {
        if (debounceTimer) clearTimeout(debounceTimer)
        debounceTimer = setTimeout(() => {
            void flushSettings().catch(error => {
                saveError.value = error instanceof Error ? error.message : String(error)
            })
        }, 500)
    }

    /** 快照只覆盖未编辑字段；正在编辑的值留在当前页面等待提交或冲突选择。 */
    function applyServerSnapshot(snapshot: settingsApi.SettingsSnapshot) {
        if (snapshot.revision < revision) return
        const previous = remote
        remote = snapshot.settings ?? {}
        revision = snapshot.revision
        applyingSnapshot = true
        try {
            for (const key of persistedKeys) {
                const current = localValue(key)
                const next = effectiveRemote(key)
                if (dirty.has(key)) {
                    // 其他窗口修改了本页待提交的字段时，不能静默覆盖任一方。
                    if (
                        !sameValue(previous[key], remote[key])
                        && !sameValue(current, next)
                        && !sameValue(remote[key], pendingWrites.get(key))
                    ) {
                        if (!conflictFields.value.includes(key)) {
                            conflictFields.value = [...conflictFields.value, key]
                        }
                    }
                    if (sameValue(current, next)) {
                        dirty.delete(key)
                        conflictFields.value = conflictFields.value.filter(field => field !== key)
                    }
                    continue
                }
                // 后端可能保存旧版或缺字段的降级顺序，展示时统一补足新音质。
                setLocalValue(key, next)
            }
        } finally {
            applyingSnapshot = false
            loaded = true
        }
    }

    function readWebLocalSettings(legacySettings: Partial<Settings>) {
        if (native) return
        applyingSnapshot = true
        try {
            for (const key of WEB_LOCAL_KEYS) {
                const raw = localStorage.getItem(WEB_LOCAL_PREFIX + key)
                // 旧网页版把这两个页面偏好也写进服务端；首次升级时迁入本浏览器。
                try {
                    const value = raw === null ? legacySettings[key] : JSON.parse(raw)
                    if (value === undefined) continue
                    setLocalValue(key, value)
                    if (raw === null) {
                        localStorage.setItem(WEB_LOCAL_PREFIX + key, JSON.stringify(value))
                    }
                } catch {
                    localStorage.removeItem(WEB_LOCAL_PREFIX + key)
                }
            }
        } finally {
            applyingSnapshot = false
        }
    }

    async function loadSettings() {
        try {
            const snapshot = await settingsApi.loadSettings()
            readWebLocalSettings(snapshot.settings)
            applyServerSnapshot(snapshot)
            saveError.value = ''
        } catch (error) {
            saveError.value = error instanceof Error ? error.message : String(error)
            throw error
        }
    }

    async function getDefaultDownloadDir() {
        try {
            const directory = await settingsApi.getDefaultDownloadDir()
            // Web 路径是服务端启动参数；Tauri 初始目录仍由原生端提供。
            applyingSnapshot = !native
            settings.value.downloadDir = directory
        } catch {
            // 目录查询失败时仍展示当前快照，不触发设置写入。
        } finally {
            applyingSnapshot = false
        }
    }

    /** 逐字段提交，让一个字段的冲突不阻塞其他设置。 */
    async function performFlush() {
        for (const key of [...dirty]) {
            if (conflictFields.value.includes(key)) continue
            const value = localValue(key)
            const expected = remote[key] ?? null
            pendingWrites.set(key, value)
            try {
                const snapshot = await settingsApi.patchSettings(
                    { [key]: value } as Partial<Settings>,
                    { [key]: expected } as Record<string, null>,
                )
                saveError.value = ''
                applyServerSnapshot(snapshot)
                // 用户在请求过程中可能又改了这个字段；此时继续保留待提交状态。
                if (sameValue(localValue(key), value) && sameValue(effectiveRemote(key), value)) {
                    dirty.delete(key)
                }
            } catch (error) {
                if (error instanceof settingsApi.SettingsConflictError) {
                    applyServerSnapshot(error.snapshot)
                    if (
                        !sameValue(localValue(key), effectiveRemote(key))
                        && !conflictFields.value.includes(key)
                    ) {
                        conflictFields.value = [...conflictFields.value, key]
                    }
                    continue
                }
                saveError.value = error instanceof Error ? error.message : String(error)
                throw error
            } finally {
                pendingWrites.delete(key)
            }
        }
    }

    async function flushSettings() {
        if (debounceTimer) {
            clearTimeout(debounceTimer)
            debounceTimer = null
        }
        if (activeFlush) {
            await activeFlush
        }
        if (dirty.size === 0) return
        activeFlush = performFlush()
        try {
            await activeFlush
        } finally {
            activeFlush = null
        }
    }

    function resolveConflict(key: SettingKey, keepMine: boolean) {
        if (!conflictFields.value.includes(key)) return
        conflictFields.value = conflictFields.value.filter(field => field !== key)
        if (keepMine) {
            // 下一次提交以最新服务端值为原值，明确表达覆盖意图。
            dirty.add(key)
            scheduleFlush()
        } else {
            applyingSnapshot = true
            setLocalValue(key, effectiveRemote(key))
            applyingSnapshot = false
            dirty.delete(key)
        }
    }

    watch(settings, () => {
        if (applyingSnapshot || !loaded) return
        if (!native) {
            for (const key of WEB_LOCAL_KEYS) {
                localStorage.setItem(WEB_LOCAL_PREFIX + key, JSON.stringify(localValue(key)))
            }
        }
        for (const key of persistedKeys) {
            if (sameValue(localValue(key), effectiveRemote(key))) {
                dirty.delete(key)
                conflictFields.value = conflictFields.value.filter(field => field !== key)
            } else {
                dirty.add(key)
            }
        }
        if (dirty.size > 0) scheduleFlush()
    }, { deep: true, flush: 'sync' })

    return {
        settings,
        conflictFields,
        saveError,
        loadSettings,
        applyServerSnapshot,
        getDefaultDownloadDir,
        flushSettings,
        resolveConflict,
    }
})

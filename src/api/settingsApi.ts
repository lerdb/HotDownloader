import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@tauri-apps/api/core'
import type { Settings } from '../types'
import { webRequest } from './webClient'

/** 设置存储仍由 Rust 管理；这里处理现有 IPC 的 JSON 字符串格式。 */
export async function loadSettings(): Promise<Partial<Settings> | null> {
    if (!isTauri()) {
        return webRequest<Partial<Settings>>('/api/settings')
    }
    const json = await invoke<string>('load_settings')
    return json ? JSON.parse(json) as Partial<Settings> : null
}

export function saveSettings(settings: Settings): Promise<void> {
    if (!isTauri()) {
        return webRequest<void>('/api/settings', {
            method: 'PUT',
            body: JSON.stringify(settings),
        })
    }
    return invoke<void>('save_settings', {
        settingsJson: JSON.stringify(settings),
    })
}

export function getDefaultDownloadDir(): Promise<string> {
    if (!isTauri()) {
        return webRequest('/api/settings/default-download-dir')
    }
    return invoke<string>('get_default_download_dir')
}

export function setMaxConcurrent(max: number): Promise<void> {
    if (!isTauri()) {
        // Web 的设置保存接口会在同一次请求中更新调度器并发数。
        return Promise.resolve()
    }
    return invoke<void>('set_max_concurrent', { max })
}

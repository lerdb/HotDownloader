import { invoke } from '@tauri-apps/api/core'
import type { Settings } from '../types'

/** 设置存储仍由 Rust 管理；这里处理现有 IPC 的 JSON 字符串格式。 */
export async function loadSettings(): Promise<Partial<Settings> | null> {
    const json = await invoke<string>('load_settings')
    return json ? JSON.parse(json) as Partial<Settings> : null
}

export function saveSettings(settings: Settings): Promise<void> {
    return invoke<void>('save_settings', {
        settingsJson: JSON.stringify(settings),
    })
}

export function getDefaultDownloadDir(): Promise<string> {
    return invoke<string>('get_default_download_dir')
}

export function setMaxConcurrent(max: number): Promise<void> {
    return invoke<void>('set_max_concurrent', { max })
}

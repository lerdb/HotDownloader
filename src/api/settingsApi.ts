import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@tauri-apps/api/core'
import type { Settings } from '../types'
import { WebRequestError, webRequest } from './webClient'

/** Rust 返回带修订号的可见设置；凭据和内部存储字段不会进入页面。 */
export interface SettingsSnapshot {
    revision: number
    settings: Partial<Settings>
}

export class SettingsConflictError extends Error {
    readonly fields: string[]
    readonly snapshot: SettingsSnapshot

    constructor(fields: string[], snapshot: SettingsSnapshot) {
        super('设置已被其他页面更新')
        this.fields = fields
        this.snapshot = snapshot
    }
}

export async function loadSettings(): Promise<SettingsSnapshot> {
    if (!isTauri()) {
        return webRequest<SettingsSnapshot>('/api/settings')
    }
    return invoke<SettingsSnapshot>('get_settings_snapshot')
}

/** 提交字段及读取时的原值。相同字段发生并发修改时，保留快照交给页面决策。 */
export async function patchSettings(
    changes: Partial<Settings>,
    expected: Partial<Settings> | Record<string, null>,
): Promise<SettingsSnapshot> {
    const patch = { changes, expected }
    try {
        if (!isTauri()) {
            return await webRequest<SettingsSnapshot>('/api/settings', {
                method: 'PATCH',
                body: JSON.stringify(patch),
            })
        }
        return await invoke<SettingsSnapshot>('patch_settings', { patch })
    } catch (error) {
        // Tauri command 的错误是 JSON 字符串，HTTP 的 409 则保留了解析后的响应体。
        const response = error instanceof WebRequestError
            ? error.body
            : parseTauriError(error)
        if (isConflictResponse(response)) {
            throw new SettingsConflictError(response.fields, response.snapshot)
        }
        throw error
    }
}

function parseTauriError(error: unknown): unknown {
    try {
        return JSON.parse(String(error)) as unknown
    } catch {
        return null
    }
}

function isConflictResponse(value: unknown): value is {
    fields: string[]
    snapshot: SettingsSnapshot
} {
    if (typeof value !== 'object' || value === null) return false
    const candidate = value as { fields?: unknown; snapshot?: unknown }
    return Array.isArray(candidate.fields) && typeof candidate.snapshot === 'object'
}

export function getDefaultDownloadDir(): Promise<string> {
    if (!isTauri()) {
        return webRequest('/api/settings/default-download-dir')
    }
    return invoke<string>('get_default_download_dir')
}

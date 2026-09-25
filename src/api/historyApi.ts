import { invoke } from '@tauri-apps/api/core'

/** 历史记录的序列化细节留在 API 层，store 只处理关键词列表。 */
export async function loadHistory(): Promise<string[]> {
    const json = await invoke<string>('load_history')
    if (!json) {
        return []
    }

    const parsed: unknown = JSON.parse(json)
    return Array.isArray(parsed) ? parsed as string[] : []
}

export function saveHistory(history: string[]): Promise<void> {
    return invoke<void>('save_history', {
        historyJson: JSON.stringify(history),
    })
}

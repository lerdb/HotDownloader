import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@tauri-apps/api/core'

const WEB_HISTORY_KEY = 'hotdownloader-search-history'

/** 历史记录的序列化细节留在 API 层，store 只处理关键词列表。 */
export async function loadHistory(): Promise<string[]> {
    if (!isTauri()) {
        const raw = localStorage.getItem(WEB_HISTORY_KEY)
        if (!raw) return []
        try {
            const parsed: unknown = JSON.parse(raw)
            return Array.isArray(parsed) ? parsed as string[] : []
        } catch {
            return []
        }
    }
    const json = await invoke<string>('load_history')
    if (!json) {
        return []
    }

    const parsed: unknown = JSON.parse(json)
    return Array.isArray(parsed) ? parsed as string[] : []
}

export function saveHistory(history: string[]): Promise<void> {
    if (!isTauri()) {
        // 搜索历史属于浏览器个人数据，不参与服务端任务状态或凭据存储。
        localStorage.setItem(WEB_HISTORY_KEY, JSON.stringify(history))
        return Promise.resolve()
    }
    return invoke<void>('save_history', {
        historyJson: JSON.stringify(history),
    })
}

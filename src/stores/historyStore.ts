import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as historyApi from '../api/historyApi'

export const useHistoryStore = defineStore('history', () => {
    const history = ref<string[]>([])

    async function loadHistory() {
        try {
            history.value = (await historyApi.loadHistory()).slice(0, 50)
        } catch {
            history.value = []
        }
    }

    async function saveHistory() {
        try {
            await historyApi.saveHistory(history.value)
        } catch (e) {
            console.error('保存搜索历史失败:', e)
        }
    }

    function addHistory(keyword: string) {
        const trimmed = keyword.trim()
        if (!trimmed) return

        // 去重
        history.value = history.value.filter((item) => item !== trimmed)
        // 插入首部
        history.value.unshift(trimmed)
        // 截断至 50
        if (history.value.length > 50) {
            history.value = history.value.slice(0, 50)
        }
        saveHistory()
    }

    function clearHistory() {
        history.value = []
        saveHistory()
    }

    function removeHistoryItem(keyword: string) {
        history.value = history.value.filter((item) => item !== keyword)
        saveHistory()
    }

    return {
        history,
        loadHistory,
        addHistory,
        clearHistory,
        removeHistoryItem,
    }
})

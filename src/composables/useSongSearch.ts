import { ref } from 'vue'
import type { SongInfo } from '../types'
import * as musicApi from '../api/musicApi'

/**
 * 歌曲搜索逻辑封装。
 * 管理歌曲搜索结果、选择状态、分页加载等，减少 SearchView 的复杂度。
 * 使用方式：在组件中调用 useSongSearch，传入响应式的平台和关键词 getter，
 * 或直接在方法中传递参数。
 */
export function useSongSearch() {
    // 搜索结果列表
    const searchResults = ref<SongInfo[]>([])
    // 选中歌曲的 mid 集合
    const selectedIds = ref<string[]>([])
    // 是否正在搜索
    const loading = ref(false)
    // 是否已经搜索过（用于显示搜索结果区域）
    const hasSearched = ref(false)
    // 是否还有更多结果
    const hasMore = ref(false)
    // 是否正在加载更多
    const loadingMore = ref(false)
    // 当前页码
    const currentPage = ref(1)
    // 每页数量
    const PAGE_SIZE = 20

    /**
     * 执行歌曲搜索。
     * @param platform 平台标识
     * @param keyword 搜索关键词
     * @param addHistory 是否添加到历史记录（由调用方决定，这里不直接操作 history store）
     */
    async function searchSongs(platform: string, keyword: string, addHistory?: (term: string) => void) {
        const term = keyword.trim()
        if (!term) return

        loading.value = true
        hasSearched.value = true
        selectedIds.value = []
        currentPage.value = 1
        hasMore.value = false
        loadingMore.value = false

        try {
            const response = await musicApi.searchSongs(platform, term, currentPage.value, PAGE_SIZE)
            searchResults.value = response.songs
            hasMore.value = response.has_more
            addHistory?.(term)
        } catch (error) {
            console.error('搜索失败:', error)
            searchResults.value = []
            hasMore.value = false
        } finally {
            loading.value = false
        }
    }

    /**
     * 加载更多歌曲。
     * @param platform 平台标识
     * @param keyword 当前搜索关键词
     */
    async function loadMoreSongs(platform: string, keyword: string) {
        if (loading.value || loadingMore.value || !hasMore.value) return

        const nextPage = currentPage.value + 1
        loadingMore.value = true

        try {
            const response = await musicApi.searchSongs(platform, keyword.trim(), nextPage, PAGE_SIZE)
            const more = response.songs

            // 去重，避免接口重复数据
            const existingIds = new Set(searchResults.value.map((s) => s.mid))
            const newSongs = more.filter((s) => !existingIds.has(s.mid))
            searchResults.value = [...searchResults.value, ...newSongs]

            currentPage.value = nextPage
            hasMore.value = response.has_more
        } catch (error) {
            console.error('加载更多失败:', error)
        } finally {
            loadingMore.value = false
        }
    }

    /**
     * 清空歌曲搜索状态（当关键词被清空或切换搜索类型时调用）。
     */
    function reset() {
        loading.value = false
        hasSearched.value = false
        searchResults.value = []
        selectedIds.value = []
        currentPage.value = 1
        hasMore.value = false
        loadingMore.value = false
    }

    return {
        searchResults,
        selectedIds,
        loading,
        hasSearched,
        hasMore,
        loadingMore,
        currentPage,
        searchSongs,
        loadMoreSongs,
        reset,
    }
}
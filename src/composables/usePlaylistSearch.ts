import { ref } from 'vue'
import type { PlaylistSearchItem } from '../types'
import * as musicApi from '../api/musicApi'

/**
 * 歌单搜索逻辑封装。
 * 管理歌单搜索结果、分页加载等。
 */
export function usePlaylistSearch() {
    // 歌单搜索结果列表
    const playlists = ref<PlaylistSearchItem[]>([])
    // 是否正在搜索
    const loading = ref(false)
    // 是否已经搜索过
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
     * 执行歌单搜索。
     * @param platform 平台标识
     * @param keyword 搜索关键词
     */
    async function searchPlaylists(platform: string, keyword: string) {
        const term = keyword.trim()
        if (!term) return

        loading.value = true
        hasSearched.value = true
        playlists.value = []
        hasMore.value = false
        currentPage.value = 1

        try {
            const res = await musicApi.searchPlaylists(platform, term, currentPage.value, PAGE_SIZE)
            playlists.value = res.playlists
            hasMore.value = res.has_more
        } catch (error) {
            console.error('搜索歌单失败:', error)
            playlists.value = []
            hasMore.value = false
        } finally {
            loading.value = false
        }
    }

    /**
     * 加载更多歌单。
     * @param platform 平台标识
     * @param keyword 当前搜索关键词
     */
    async function loadMorePlaylists(platform: string, keyword: string) {
        if (loading.value || loadingMore.value || !hasMore.value) return

        const nextPage = currentPage.value + 1
        loadingMore.value = true

        try {
            const res = await musicApi.searchPlaylists(platform, keyword.trim(), nextPage, PAGE_SIZE)
            const more = res.playlists

            // 去重
            const existingIds = new Set(playlists.value.map((p) => p.id))
            const newPlaylists = more.filter((p) => !existingIds.has(p.id))
            playlists.value = [...playlists.value, ...newPlaylists]

            currentPage.value = nextPage
            hasMore.value = res.has_more
        } catch (error) {
            console.error('加载更多歌单失败:', error)
        } finally {
            loadingMore.value = false
        }
    }

    /**
     * 清空歌单搜索状态。
     */
    function reset() {
        loading.value = false
        hasSearched.value = false
        playlists.value = []
        hasMore.value = false
        loadingMore.value = false
        currentPage.value = 1
    }

    return {
        playlists,
        loading,
        hasSearched,
        hasMore,
        loadingMore,
        currentPage,
        searchPlaylists,
        loadMorePlaylists,
        reset,
    }
}
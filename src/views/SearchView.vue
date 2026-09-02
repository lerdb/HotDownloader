<template>
    <div class="search-view">
        <!-- 平台绑定 -->
        <SearchBar v-model:keyword="keyword" v-model:platform="currentPlatform" :platform-options="PLATFORMS"
            @search="handleSearch" />

        <!-- 输入非空且未搜索：显示搜索建议 -->
        <SearchSuggestions v-if="showSuggestions" :data="suggestions" @select="onSuggestionSelect" />

        <!-- 输入为空且未搜索：显示历史与热搜 -->
        <div v-if="!keyword && !hasSearched">
            <SearchHistory :history="historyStore.history" @select="onHistorySelect" @remove="onHistoryRemove"
                @clear="historyStore.clearHistory" />
            <HotKeywords :keywords="hotKeywords" :loading="hotLoading" @select="onHotClick" />
        </div>

        <!-- 加载中 -->
        <div v-if="loading" class="loading-wrapper">
            <n-spin size="medium" />
        </div>

        <!-- 搜索结果列表（已搜索完毕），支持分页加载更多 -->
        <SearchResultList v-if="hasSearched && !loading" :songs="searchResults" v-model:selectedIds="selectedIds"
            :has-more="hasMore" :loading-more="loadingMore" @download="onSingleDownload" @retry="handleSearch"
            @load-more="loadMore" />

        <BatchDownloadBar v-if="selectedIds.length > 0" :selectedCount="selectedIds.length"
            @batch-download="onBatchDownload" />
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted } from 'vue'
import { NSpin } from 'naive-ui'
import SearchBar from '../components/search/SearchBar.vue'
import SearchHistory from '../components/search/SearchHistory.vue'
import HotKeywords from '../components/search/HotKeywords.vue'
import SearchSuggestions from '../components/search/SearchSuggestions.vue'
import SearchResultList from '../components/search/SearchResultList.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import { useHistoryStore } from '../stores/historyStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import * as musicApi from '../api/musicApi'
import type { SongInfo, SearchSuggestionData } from '../types'
import { PLATFORMS, DEFAULT_PLATFORM } from '../config/platforms'

const keyword = ref('')
const searchResults = ref<SongInfo[]>([])
const selectedIds = ref<string[]>([])
const loading = ref(false)
const hasSearched = ref(false)
const currentPlatform = ref(DEFAULT_PLATFORM) // 当前平台

// ==================== 分页加载更多状态 ====================
// 每页数量与后端 search_songs 的 limit 参数保持一致
const PAGE_SIZE = 20
// 当前已加载到第几页，新搜索时重置为 1
const currentPage = ref(1)
// 是否还有更多搜索结果，由后端返回的 has_more 字段决定
const hasMore = ref(false)
// 是否正在请求加载下一页，防止重复点击
const loadingMore = ref(false)

// 热搜
const hotKeywords = ref<string[]>([])
const hotLoading = ref(false)

const historyStore = useHistoryStore()
const { downloadSingle, batchDownload } = useDownloadActions()

// ==================== 搜索建议相关 ====================
const suggestions = ref<SearchSuggestionData>({
    song: [],
    singer: [],
    album: [],
    mv: [],
})

let abortController: AbortController | null = null
let debounceTimer: ReturnType<typeof setTimeout> | null = null

// 是否显示建议：关键词非空且未进入搜索结果页
const showSuggestions = computed(() => {
    return keyword.value.trim() !== '' && !hasSearched.value
})

// 防抖请求建议
watch(keyword, (newVal) => {
    if (debounceTimer) {
        clearTimeout(debounceTimer)
    }
    if (abortController) {
        abortController.abort() // 取消上次请求
    }

    const term = newVal.trim()
    if (!term) {
        suggestions.value = { song: [], singer: [], album: [], mv: [] }
        return
    }

    debounceTimer = setTimeout(async () => {
        const controller = new AbortController()
        abortController = controller
        try {
            const res = await musicApi.fetchSuggestions(currentPlatform.value, term)
            if (!controller.signal.aborted) {
                suggestions.value = res
            }
        } catch {
            if (!controller.signal.aborted) {
                suggestions.value = { song: [], singer: [], album: [], mv: [] }
            }
        } finally {
            if (abortController === controller) {
                abortController = null
            }
        }
    }, 300)
})

// 点击建议项
function onSuggestionSelect(word: string) {
    keyword.value = word
    handleSearch()
}
// ==================== 建议逻辑结束 ====================

// 关键词清空时重置状态
watch(keyword, (newVal) => {
    if (!newVal) {
        hasSearched.value = false
        searchResults.value = []
        selectedIds.value = []
        suggestions.value = { song: [], singer: [], album: [], mv: [] }
        currentPage.value = 1
        hasMore.value = false
        loadingMore.value = false
    }
})

// 获取热搜
async function fetchHotKeywords() {
    hotLoading.value = true
    try {
        hotKeywords.value = await musicApi.getHotKeywords(currentPlatform.value)
    } catch {
        hotKeywords.value = []
    } finally {
        hotLoading.value = false
    }
}

onMounted(() => {
    fetchHotKeywords()
})

// 热搜点击
function onHotClick(word: string) {
    keyword.value = word
    handleSearch()
}

// 搜索历史点击
function onHistorySelect(term: string) {
    keyword.value = term
    handleSearch()
}

function onHistoryRemove(term: string) {
    historyStore.removeHistoryItem(term)
}

async function handleSearch() {
    const term = keyword.value.trim()
    if (!term) return

    loading.value = true
    hasSearched.value = true
    selectedIds.value = []
    currentPage.value = 1
    hasMore.value = false
    loadingMore.value = false

    try {
        const response = await musicApi.searchSongs(currentPlatform.value, term, currentPage.value, PAGE_SIZE)
        searchResults.value = response.songs
        hasMore.value = response.has_more
        historyStore.addHistory(term)
    } catch (error) {
        console.error('搜索失败:', error)
        searchResults.value = []
        hasMore.value = false
    } finally {
        loading.value = false
    }
}

// 加载更多搜索结果
// 修改点：使用新的 SearchResponse 返回结构，并基于后端返回的 has_more 更新按钮状态
async function loadMore() {
    if (loading.value || loadingMore.value || !hasMore.value) return

    const nextPage = currentPage.value + 1
    loadingMore.value = true

    try {
        const response = await musicApi.searchSongs(currentPlatform.value, keyword.value.trim(), nextPage, PAGE_SIZE)
        const more = response.songs

        // 按歌曲 id 去重，避免接口偶发重复数据导致列表混乱
        const existingIds = new Set(searchResults.value.map((s) => s.mid))
        const newSongs = more.filter((s) => !existingIds.has(s.mid))
        searchResults.value = [...searchResults.value, ...newSongs]

        currentPage.value = nextPage
        hasMore.value = response.has_more
    } catch (error) {
        console.error('加载更多失败:', error)
        // 保留 hasMore 状态，允许用户再次点击加载更多进行重试
    } finally {
        loadingMore.value = false
    }
}

function onSingleDownload(song: SongInfo) {
    downloadSingle(song)
}

function onBatchDownload() {
    const songs = searchResults.value.filter((s) => selectedIds.value.includes(s.mid))
    if (songs.length > 0) {
        batchDownload(songs)
    }
}
</script>

<style scoped>
.search-view {
    display: flex;
    flex-direction: column;
    /* 防止底部导航遮挡 */
    min-height: 100%;
    padding-bottom: 0;
}

.loading-wrapper {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}
</style>
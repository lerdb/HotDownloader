<template>
    <div class="search-view">
        <!-- 平台绑定 + 搜索类型切换 -->
        <div class="search-header">
            <SearchBar v-model:keyword="keyword" v-model:platform="currentPlatform" :platform-options="PLATFORMS"
                :placeholder="searchType === 'song' ? '搜索歌曲、歌手、专辑' : '输入关键词搜索歌单'"
                :button-text="searchType === 'song' ? '搜索' : '搜索歌单'" @search="handleSearch" />

            <!-- 搜索类型切换按钮 -->
            <div class="type-switch">
                <n-button quaternary :type="searchType === 'song' ? 'primary' : 'default'"
                    @click="switchSearchType('song')">歌曲</n-button>
                <n-button quaternary :type="searchType === 'playlist' ? 'primary' : 'default'"
                    @click="switchSearchType('playlist')">歌单</n-button>
            </div>
        </div>

        <!-- 歌曲搜索模式下的建议/历史/热搜 -->
        <template v-if="searchType === 'song'">
            <SearchSuggestions v-if="showSuggestions" :data="suggestions" @select="onSuggestionSelect" />

            <div v-if="!keyword && !songHasSearched">
                <SearchHistory :history="historyStore.history" @select="onHistorySelect" @remove="onHistoryRemove"
                    @clear="historyStore.clearHistory" />
                <HotKeywords :keywords="hotKeywords" :loading="hotLoading" @select="onHotClick" />
            </div>
        </template>

        <!-- 加载中 -->
        <div v-if="(searchType === 'song' ? songLoading : playlistLoading)" class="loading-wrapper">
            <n-spin size="medium" />
        </div>

        <!-- 歌曲搜索结果列表 -->
        <SearchResultList v-if="searchType === 'song' && songHasSearched && !songLoading" :songs="songSearchResults"
            v-model:selectedIds="songSelectedIds" :has-more="songHasMore" :loading-more="songLoadingMore"
            @download="onSingleDownload" @retry="handleSearch" @load-more="loadMoreSongs" />

        <!-- 歌单搜索结果列表 -->
        <PlaylistSearchResult v-if="searchType === 'playlist' && playlistHasSearched && !playlistLoading"
            :playlists="playlistSearchResults" :has-more="playlistHasMore" :loading-more="playlistLoadingMore"
            @click-playlist="goToPlaylist" @load-more="loadMorePlaylists" />

        <!-- 批量下载栏（仅在歌曲搜索模式且有选中时显示） -->
        <BatchDownloadBar v-if="searchType === 'song' && songSelectedIds.length > 0"
            :selectedCount="songSelectedIds.length" @batch-download="onBatchDownload" />
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted } from 'vue'
import { NSpin, NButton } from 'naive-ui'
import { useRouter } from 'vue-router'
import SearchBar from '../components/search/SearchBar.vue'
import SearchHistory from '../components/search/SearchHistory.vue'
import HotKeywords from '../components/search/HotKeywords.vue'
import SearchSuggestions from '../components/search/SearchSuggestions.vue'
import SearchResultList from '../components/search/SearchResultList.vue'
import PlaylistSearchResult from '../components/search/PlaylistSearchResult.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import { useHistoryStore } from '../stores/historyStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useSongSearch } from '../composables/useSongSearch'
import { usePlaylistSearch } from '../composables/usePlaylistSearch'
import * as musicApi from '../api/musicApi'
import type { SearchSuggestionData, PlaylistSearchItem, SongInfo } from '../types'
import { PLATFORMS, DEFAULT_PLATFORM } from '../config/platforms'

const router = useRouter()
const keyword = ref('')
const currentPlatform = ref(DEFAULT_PLATFORM)

// 搜索类型
type SearchType = 'song' | 'playlist'
const searchType = ref<SearchType>('song')

// 使用歌曲搜索 composable，解构出状态和方法
const {
    searchResults: songSearchResults,
    selectedIds: songSelectedIds,
    loading: songLoading,
    hasSearched: songHasSearched,
    hasMore: songHasMore,
    loadingMore: songLoadingMore,
    searchSongs: searchSongFunc,
    loadMoreSongs: loadMoreSongsFunc,
    reset: resetSongSearch,
} = useSongSearch()

// 使用歌单搜索 composable，解构出状态和方法
const {
    playlists: playlistSearchResults,
    loading: playlistLoading,
    hasSearched: playlistHasSearched,
    hasMore: playlistHasMore,
    loadingMore: playlistLoadingMore,
    searchPlaylists: searchPlaylistFunc,
    loadMorePlaylists: loadMorePlaylistFunc,
    reset: resetPlaylistSearch,
} = usePlaylistSearch()

// 历史与热搜
const historyStore = useHistoryStore()
const hotKeywords = ref<string[]>([])
const hotLoading = ref(false)

// 下载操作
const { downloadSingle, batchDownload } = useDownloadActions()

// 搜索建议相关
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
    return keyword.value.trim() !== '' && !songHasSearched.value
})

// 防抖获取建议
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

// 关键词清空时重置
watch(keyword, (newVal) => {
    if (!newVal) {
        resetSongSearch()
        resetPlaylistSearch()
        suggestions.value = { song: [], singer: [], album: [], mv: [] }
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

// 平台切换
watch(currentPlatform, () => {
    fetchHotKeywords()
    suggestions.value = { song: [], singer: [], album: [], mv: [] }
    if (abortController) abortController.abort()
    resetSongSearch()
    resetPlaylistSearch()
})

// 切换搜索类型
function switchSearchType(type: SearchType) {
    searchType.value = type
    const term = keyword.value.trim()
    if (term) {
        if (type === 'song') {
            searchSongFunc(currentPlatform.value, term, historyStore.addHistory)
        } else {
            searchPlaylistFunc(currentPlatform.value, term)
        }
    }
}

// 热搜点击
function onHotClick(word: string) {
    keyword.value = word
    handleSearch()
}

// 历史点击
function onHistorySelect(term: string) {
    keyword.value = term
    handleSearch()
}

function onHistoryRemove(term: string) {
    historyStore.removeHistoryItem(term)
}

// 统一搜索入口
async function handleSearch() {
    const term = keyword.value.trim()
    if (!term) return

    if (searchType.value === 'song') {
        await searchSongFunc(currentPlatform.value, term, historyStore.addHistory)
    } else {
        await searchPlaylistFunc(currentPlatform.value, term)
    }
}

// 加载更多歌曲
function loadMoreSongs() {
    loadMoreSongsFunc(currentPlatform.value, keyword.value)
}

// 加载更多歌单
function loadMorePlaylists() {
    loadMorePlaylistFunc(currentPlatform.value, keyword.value)
}

// 跳转歌单详情
function goToPlaylist(pl: PlaylistSearchItem) {
    router.push({ path: '/playlist', query: { platform: currentPlatform.value, id: pl.id } })
}

// 单曲下载
function onSingleDownload(song: SongInfo) {
    downloadSingle(song)
}

// 批量下载
function onBatchDownload() {
    const songs = songSearchResults.value.filter((s) => songSelectedIds.value.includes(s.mid))
    if (songs.length > 0) {
        batchDownload(songs)
    }
}
</script>

<style scoped>
.search-view {
    display: flex;
    flex-direction: column;
    min-width: 0;
    /* 防止底部导航遮挡 */
    min-height: 100%;
    padding-bottom: 0;
}

.search-header {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 16px;
}

.type-switch {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
}

/* 搜索栏和类型切换的间距由 search-header 统一控制 */
.search-header :deep(.search-bar) {
    margin-bottom: 0;
}

@media (max-width: 767px) {
    .type-switch .n-button {
        min-height: 44px;
    }
}

.loading-wrapper {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}
</style>

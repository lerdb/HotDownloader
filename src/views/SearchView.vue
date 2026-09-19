<template>
    <div class="search-view">
        <!-- 平台绑定 + 搜索类型切换 -->
        <div class="search-header">
            <SearchBar :keyword="keyword" @update:keyword="onKeywordInput" v-model:platform="currentPlatform"
                :platform-options="PLATFORMS" :placeholder="searchPlaceholder" button-text="搜索"
                @search="handleSearch" />

            <!-- 搜索类型切换按钮 -->
            <div class="type-switch">
                <n-button quaternary :type="searchType === 'song' ? 'primary' : 'default'"
                    @click="switchSearchType('song')">歌曲</n-button>
                <n-button quaternary :type="searchType === 'artist' ? 'primary' : 'default'"
                    @click="switchSearchType('artist')">歌手</n-button>
                <n-button quaternary :type="searchType === 'album' ? 'primary' : 'default'"
                    @click="switchSearchType('album')">专辑</n-button>
                <n-button quaternary :type="searchType === 'playlist' ? 'primary' : 'default'"
                    @click="switchSearchType('playlist')">歌单</n-button>
            </div>
        </div>

        <!-- 空闲：历史与热搜 -->
        <template v-if="pageMode === 'idle'">
            <SearchHistory :history="historyStore.history" @select="onHistorySelect" @remove="onHistoryRemove"
                @clear="historyStore.clearHistory" />
            <HotKeywords :keywords="hotKeywords" :loading="hotLoading" @select="onHotClick" />
        </template>

        <!-- 输入中：搜索建议 -->
        <template v-else-if="pageMode === 'suggestions'">
            <SearchSuggestions :data="suggestions" @select="onSuggestionSelect" />
        </template>

        <!-- 已搜索：搜索结果 -->
        <template v-else-if="pageMode === 'results'">
            <!-- 加载中 -->
            <div v-if="searchLoading" class="loading-wrapper">
                <n-spin size="medium" />
            </div>

            <!-- 歌曲搜索结果列表 -->
            <SearchResultList v-if="searchType === 'song' && songHasSearched && !songLoading" :songs="songSearchResults"
                v-model:selectedIds="songSelectedIds" :has-more="songHasMore" :loading-more="songLoadingMore"
                @download="onSingleDownload" @retry="handleSearch" @load-more="loadMoreSongs" />

            <!-- 歌手搜索结果列表 -->
            <template v-else-if="searchType === 'artist'">
                <n-alert v-if="artistError" type="error" title="歌手搜索失败">
                    {{ artistError }}
                    <n-button @click="artistSearchResults.length ? loadMoreArtists() : handleSearch()">重试</n-button>
                </n-alert>
                <ArtistSearchResult
                    v-if="artistHasSearched && !artistLoading && (!artistError || artistSearchResults.length)"
                    :artists="artistSearchResults" :has-more="artistHasMore" :loading-more="artistLoadingMore"
                    @click-artist="goToArtist" @load-more="loadMoreArtists" />
            </template>

            <!-- 专辑搜索结果列表 -->
            <template v-else-if="searchType === 'album'">
                <n-alert v-if="albumError" type="error" title="专辑搜索失败" class="album-error">
                    {{ albumError }}
                    <n-button @click="albumSearchResults.length ? loadMoreAlbums() : handleSearch()">重试</n-button>
                </n-alert>
                <AlbumSearchResult
                    v-if="albumHasSearched && !albumLoading && (!albumError || albumSearchResults.length)"
                    :albums="albumSearchResults" :has-more="albumHasMore" :loading-more="albumLoadingMore"
                    @click-album="goToAlbum" @load-more="loadMoreAlbums" />
            </template>

            <!-- 歌单搜索结果列表 -->
            <PlaylistSearchResult v-else-if="searchType === 'playlist' && playlistHasSearched && !playlistLoading"
                :playlists="playlistSearchResults" :has-more="playlistHasMore" :loading-more="playlistLoadingMore"
                @click-playlist="goToPlaylist" @load-more="loadMorePlaylists" />

            <!-- 批量下载栏（仅在歌曲搜索模式且有选中时显示） -->
            <BatchDownloadBar v-if="searchType === 'song' && songSelectedIds.length > 0"
                :selectedCount="songSelectedIds.length" @batch-download="onBatchDownload" />
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue'
import { NSpin, NButton, NAlert } from 'naive-ui'
import { useRouter } from 'vue-router'
import SearchBar from '../components/search/SearchBar.vue'
import SearchHistory from '../components/search/SearchHistory.vue'
import HotKeywords from '../components/search/HotKeywords.vue'
import SearchSuggestions from '../components/search/SearchSuggestions.vue'
import SearchResultList from '../components/search/SearchResultList.vue'
import ArtistSearchResult from '../components/search/ArtistSearchResult.vue'
import { useArtistSearch } from '../composables/useArtistSearch'
import AlbumSearchResult from '../components/search/AlbumSearchResult.vue'
import { useAlbumSearch } from '../composables/useAlbumSearch'
import PlaylistSearchResult from '../components/search/PlaylistSearchResult.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import { useHistoryStore } from '../stores/historyStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useSongSearch } from '../composables/useSongSearch'
import { usePlaylistSearch } from '../composables/usePlaylistSearch'
import * as musicApi from '../api/musicApi'
import type { SearchSuggestionData, PlaylistSearchItem, AlbumInfo, ArtistInfo, SongInfo } from '../types'
import { PLATFORMS, DEFAULT_PLATFORM } from '../config/platforms'

const router = useRouter()
const keyword = ref('')
const currentPlatform = ref(DEFAULT_PLATFORM)

// 搜索类型
type SearchType = 'song' | 'artist' | 'album' | 'playlist'
const searchType = ref<SearchType>('song')
const pageMode = ref<'idle' | 'suggestions' | 'results'>('idle')

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

const {
    albums: albumSearchResults,
    loading: albumLoading,
    loadingMore: albumLoadingMore,
    hasSearched: albumHasSearched,
    hasMore: albumHasMore,
    error: albumError,
    search: searchAlbumFunc,
    reset: resetAlbumSearch,
    loadMore: loadMoreAlbums,
} = useAlbumSearch()
const {
    artists: artistSearchResults,
    loading: artistLoading,
    loadingMore: artistLoadingMore,
    hasSearched: artistHasSearched,
    hasMore: artistHasMore,
    error: artistError,
    search: searchArtistFunc,
    reset: resetArtistSearch,
    loadMore: loadMoreArtists,
} = useArtistSearch()
const searchPlaceholder = computed(() => ({
    song: '搜索歌曲、歌手、专辑',
    artist: '输入关键词搜索歌手',
    album: '输入关键词搜索专辑',
    playlist: '输入关键词搜索歌单'
})[searchType.value])
const searchLoading = computed(() => ({
    song: songLoading.value,
    artist: artistLoading.value,
    album: albumLoading.value,
    playlist: playlistLoading.value
})[searchType.value])

// 历史与热搜
const historyStore = useHistoryStore()
const hotKeywords = ref<string[]>([])
const hotLoading = ref(false)

// 下载操作
const {
    downloadSingle,
    batchDownload
} = useDownloadActions()

// 搜索建议相关
const suggestions = ref<SearchSuggestionData>({
    song: [],
    singer: [],
    album: [],
    mv: [],
})

let abortController: AbortController | null = null
let debounceTimer: ReturnType<typeof setTimeout> | null = null

function cancelSuggestions() {
    if (debounceTimer) {
        clearTimeout(debounceTimer)
        debounceTimer = null
    }
    if (abortController) {
        // IPC 无法取消传输；标记失效，阻止旧响应写入页面。
        abortController.abort()
        abortController = null
    }
}

// 仅用户编辑输入框时进入建议页。
function onKeywordInput(newVal: string) {
    if (newVal === keyword.value) return
    keyword.value = newVal
    cancelSuggestions()
    resetSearches()
    const term = newVal.trim()
    pageMode.value = term ? 'suggestions' : 'idle'
    suggestions.value = {
        song: [],
        singer: [],
        album: [],
        mv: []
    }
    if (!term) return

    const platform = currentPlatform.value
    debounceTimer = setTimeout(async () => {
        debounceTimer = null
        const controller = new AbortController()
        abortController = controller
        try {
            const res = await musicApi.fetchSuggestions(platform, term)
            if (!controller.signal.aborted) {
                suggestions.value = res
            }
        } catch {
            // 输入时已清空建议，请求失败时保持为空。
        } finally {
            if (abortController === controller) {
                abortController = null
            }
        }
    }, 300)
}

// 点击建议项
function onSuggestionSelect(word: string, type: keyof SearchSuggestionData) {
    searchType.value = type === 'album' ? 'album' : type === 'singer' ? 'artist' : 'song'
    keyword.value = word
    handleSearch()
}

onBeforeUnmount(cancelSuggestions)

function resetSearches() {
    resetSongSearch()
    resetPlaylistSearch()
    resetAlbumSearch()
    resetArtistSearch()
}

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
    cancelSuggestions()
    fetchHotKeywords()
    suggestions.value = {
        song: [],
        singer: [],
        album: [],
        mv: []
    }
    resetSearches()
    if (pageMode.value === 'results') void handleSearch()
}, { flush: 'sync' })

// 切换搜索类型
function switchSearchType(type: SearchType) {
    if (type === searchType.value) return
    cancelSuggestions()
    resetSearches()
    searchType.value = type
    if (pageMode.value === 'results') void handleSearch()
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
    cancelSuggestions()
    const term = keyword.value.trim()
    if (!term) {
        pageMode.value = 'idle'
        resetSearches()
        return
    }

    pageMode.value = 'results'
    suggestions.value = { song: [], singer: [], album: [], mv: [] }
    historyStore.addHistory(term)
    if (searchType.value === 'song') {
        await searchSongFunc(currentPlatform.value, term)
    } else if (searchType.value === 'artist') {
        await searchArtistFunc(currentPlatform.value, term)
    } else if (searchType.value === 'album') {
        await searchAlbumFunc(currentPlatform.value, term)
    } else if (searchType.value === 'playlist') {
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

function goToArtist(artist: ArtistInfo) {
    router.push({
        path: '/artist',
        query: {
            platform: currentPlatform.value,
            id: artist.id,
            name: artist.name,
            cover: artist.coverUrl,
            alias: artist.alias,
            region: artist.region,
            songs: artist.songCount,
            albums: artist.albumCount
        }
    })
}

function goToAlbum(album: AlbumInfo) {
    router.push({
        path: '/album',
        query: {
            platform: currentPlatform.value,
            id: album.id,
            name: album.name,
            artist: album.artist,
            date: album.publishDate
        }
    })
}

// 跳转歌单详情
function goToPlaylist(pl: PlaylistSearchItem) {
    router.push({
        path: '/playlist',
        query: {
            platform: currentPlatform.value,
            id: pl.id
        }
    })
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

<template>
    <div class="search-view">
        <header v-if="pageMode === 'idle'" class="discover-hero">
            <div class="hero-kicker">
                <AppIcon name="sparkles" :size="14" />
                <span>探索与下载</span>
            </div>
            <h1>发现你喜欢的音乐</h1>
            <p>跨平台检索无损单曲、专辑、歌手与歌单，支持在线试听与批量下载</p>
        </header>

        <!-- 平台绑定 + 搜索类型切换 -->
        <div class="search-header">
            <SearchBar
                :keyword="keyword"
                @update:keyword="onKeywordInput"
                v-model:platform="currentPlatform"
                :platform-options="PLATFORMS"
                :placeholder="searchPlaceholder"
                button-text="搜索"
                @search="handleSearch"
            />

            <!-- SimpMusic 风格胶囊筛选芯片 -->
            <div class="type-switch" role="tablist" aria-label="搜索类型">
                <button
                    v-for="tab in searchTabs"
                    :key="tab.value"
                    type="button"
                    role="tab"
                    :aria-selected="searchType === tab.value"
                    class="type-chip"
                    :class="{ 'is-active': searchType === tab.value }"
                    @click="switchSearchType(tab.value)"
                >
                    <AppIcon :name="tab.icon" :size="15" />
                    <span>{{ tab.label }}</span>
                </button>
            </div>
        </div>

        <!-- 空闲：历史与热搜 -->
        <template v-if="pageMode === 'idle'">
            <SearchHistory
                :history="historyStore.history"
                @select="onHistorySelect"
                @remove="onHistoryRemove"
                @clear="historyStore.clearHistory"
            />
            <HotKeywords
                :keywords="hotKeywords"
                :loading="hotLoading"
                @select="onHotClick"
            />
        </template>

        <!-- 输入中：搜索建议 -->
        <template v-else-if="pageMode === 'suggestions'">
            <SearchSuggestions
                :data="suggestions"
                @select="onSuggestionSelect"
            />
        </template>

        <!-- 已搜索：搜索结果 -->
        <template v-else-if="pageMode === 'results'">
            <!-- 加载中 -->
            <div v-if="searchLoading" class="loading-wrapper">
                <n-spin size="medium" />
            </div>
            <n-alert
                v-if="searchError"
                type="error"
                :title="searchErrorTitle"
                class="search-error"
            >
                {{ searchError }}
                <n-button size="small" @click="retrySearch">重试</n-button>
            </n-alert>

            <!-- 歌曲搜索结果列表 -->
            <SearchResultList
                v-if="
                    searchType === 'song' &&
                    songHasSearched &&
                    !songLoading &&
                    (!songError || songSearchResults.length)
                "
                :songs="songSearchResults"
                v-model:selectedIds="songSelectedIds"
                :has-more="songHasMore"
                :loading-more="songLoadingMore"
                @download="onSingleDownload"
                @retry="handleSearch"
                @load-more="loadMoreSongsFunc"
                @click-artist="openRelatedArtist"
                @click-album="openSongAlbum"
            />

            <!-- 歌手搜索结果列表 -->
            <template v-else-if="searchType === 'artist'">
                <ArtistSearchResult
                    v-if="
                        artistHasSearched &&
                        !artistLoading &&
                        (!artistError || artistSearchResults.length)
                    "
                    :artists="artistSearchResults"
                    :has-more="artistHasMore"
                    :loading-more="artistLoadingMore"
                    @click-artist="
                        (artist) => openArtist(currentPlatform, artist)
                    "
                    @load-more="loadMoreArtists"
                />
            </template>

            <!-- 专辑搜索结果列表 -->
            <template v-else-if="searchType === 'album'">
                <AlbumSearchResult
                    v-if="
                        albumHasSearched &&
                        !albumLoading &&
                        (!albumError || albumSearchResults.length)
                    "
                    :albums="albumSearchResults"
                    :platform="currentPlatform"
                    :has-more="albumHasMore"
                    :loading-more="albumLoadingMore"
                    @click-album="(album) => openAlbum(currentPlatform, album)"
                    @load-more="loadMoreAlbums"
                    @click-artist="openRelatedArtist"
                />
            </template>

            <!-- 歌单搜索结果列表 -->
            <PlaylistSearchResult
                v-else-if="
                    searchType === 'playlist' &&
                    playlistHasSearched &&
                    !playlistLoading &&
                    (!playlistError || playlistSearchResults.length)
                "
                :playlists="playlistSearchResults"
                :has-more="playlistHasMore"
                :loading-more="playlistLoadingMore"
                @click-playlist="goToPlaylist"
                @load-more="loadMorePlaylistFunc"
            />

            <!-- 批量下载栏（仅在歌曲搜索模式且有选中时显示） -->
            <BatchDownloadBar
                v-if="searchType === 'song' && songSelectedIds.length > 0"
                :selectedCount="songSelectedIds.length"
                @batch-download="onBatchDownload"
            />
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue'
import { NSpin, NButton, NAlert } from 'naive-ui'
import { useRoute, useRouter } from 'vue-router'
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
import AppIcon, { type AppIconName } from '../components/common/AppIcon.vue'
import { useHistoryStore } from '../stores/historyStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useSongSearch } from '../composables/useSongSearch'
import { usePlaylistSearch } from '../composables/usePlaylistSearch'
import * as musicApi from '../api/musicApi'
import type {
    SearchSuggestionData,
    PlaylistSearchItem,
    SongInfo,
} from '../types'
import { PLATFORMS, DEFAULT_PLATFORM } from '../config/platforms'
import { useMusicNavigation } from '../composables/useMusicNavigation'

const router = useRouter()
const route = useRoute()
const { openArtist, openAlbum, openRelatedArtist, openSongAlbum } =
    useMusicNavigation()
const keyword = ref('')
const currentPlatform = ref(DEFAULT_PLATFORM)

// 搜索类型
type SearchType = 'song' | 'artist' | 'album' | 'playlist'
const searchTabs: { value: SearchType; label: string; icon: AppIconName }[] = [
    { value: 'song', label: '歌曲', icon: 'music' },
    { value: 'artist', label: '歌手', icon: 'artist' },
    { value: 'album', label: '专辑', icon: 'disc' },
    { value: 'playlist', label: '歌单', icon: 'playlist' },
]

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
    error: songError,
    retry: retrySongs,
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
    error: playlistError,
    retry: retryPlaylists,
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
    retry: retryAlbums,
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
    retry: retryArtists,
    search: searchArtistFunc,
    reset: resetArtistSearch,
    loadMore: loadMoreArtists,
} = useArtistSearch()

const searchPlaceholder = computed(
    () =>
        ({
            song: '搜索歌曲、歌手、专辑',
            artist: '输入关键词搜索歌手',
            album: '输入关键词搜索专辑',
            playlist: '输入关键词搜索歌单',
        })[searchType.value],
)
const searchLoading = computed(
    () =>
        ({
            song: songLoading.value,
            artist: artistLoading.value,
            album: albumLoading.value,
            playlist: playlistLoading.value,
        })[searchType.value],
)
const searchError = computed(
    () =>
        ({
            song: songError.value,
            artist: artistError.value,
            album: albumError.value,
            playlist: playlistError.value,
        })[searchType.value],
)
const searchErrorTitle = computed(
    () =>
        ({
            song: '歌曲搜索失败',
            artist: '歌手搜索失败',
            album: '专辑搜索失败',
            playlist: '歌单搜索失败',
        })[searchType.value],
)

function retrySearch() {
    void {
        song: retrySongs,
        artist: retryArtists,
        album: retryAlbums,
        playlist: retryPlaylists,
    }[searchType.value]()
}

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

function cancelSuggestions() {
    if (debounceTimer) {
        clearTimeout(debounceTimer)
        debounceTimer = null
    }
    if (abortController) {
        abortController.abort()
        abortController = null
    }
}

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
        mv: [],
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

function onSuggestionSelect(word: string, type: keyof SearchSuggestionData) {
    searchType.value =
        type === 'album' ? 'album' : type === 'singer' ? 'artist' : 'song'
    keyword.value = word
    handleSearch()
}

onBeforeUnmount(() => {
    cancelSuggestions()
    resetSearches()
    hotRequest++
})

function resetSearches() {
    resetSongSearch()
    resetPlaylistSearch()
    resetAlbumSearch()
    resetArtistSearch()
}

let hotRequest = 0
async function fetchHotKeywords() {
    const request = ++hotRequest
    const platform = currentPlatform.value
    hotLoading.value = true
    try {
        const result = await musicApi.getHotKeywords(platform)
        if (request === hotRequest) hotKeywords.value = result
    } catch {
        if (request === hotRequest) hotKeywords.value = []
    } finally {
        if (request === hotRequest) hotLoading.value = false
    }
}

onMounted(() => {
    fetchHotKeywords()
})

watch(
    currentPlatform,
    () => {
        cancelSuggestions()
        fetchHotKeywords()
        suggestions.value = {
            song: [],
            singer: [],
            album: [],
            mv: [],
        }
        resetSearches()
        if (pageMode.value === 'results') void handleSearch()
    },
    { flush: 'sync' },
)

function switchSearchType(type: SearchType) {
    if (type === searchType.value) return
    cancelSuggestions()
    resetSearches()
    searchType.value = type
    if (pageMode.value === 'results') void handleSearch()
}

function onHotClick(word: string) {
    keyword.value = word
    handleSearch()
}

function onHistorySelect(term: string) {
    keyword.value = term
    handleSearch()
}

function onHistoryRemove(term: string) {
    historyStore.removeHistoryItem(term)
}

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

function goToPlaylist(pl: PlaylistSearchItem) {
    router.push({
        path: '/playlist',
        query: {
            platform: currentPlatform.value,
            id: pl.id,
        },
        state: { musicReturnTo: route.fullPath },
    })
}

function onSingleDownload(song: SongInfo) {
    downloadSingle(song)
}

function onBatchDownload() {
    const songs = songSearchResults.value.filter((s) =>
        songSelectedIds.value.includes(s.mid),
    )
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
    min-height: 100%;
    padding-bottom: 0;
}

.discover-hero {
    margin-bottom: 18px;
}

.hero-kicker {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 999px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-size: 12px;
    font-weight: 600;
    margin-bottom: 10px;
}

.discover-hero h1 {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--color-text);
    margin: 0 0 6px;
}

.discover-hero p {
    font-size: 13px;
    color: var(--color-text-secondary);
    line-height: 1.5;
}

.search-header {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 18px;
}

.type-switch {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
}

.type-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 6px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--bg-sidebar);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition:
        background-color 0.18s ease,
        color 0.18s ease,
        border-color 0.18s ease;
}

.type-chip:hover {
    color: var(--color-text);
    border-color: var(--border-color-strong);
}

.type-chip.is-active {
    background: var(--color-primary-soft);
    color: var(--color-primary);
    border-color: var(--color-primary);
    font-weight: 600;
}

.search-header :deep(.search-bar) {
    margin-bottom: 0;
}

@media (max-width: 767px) {
    .discover-hero h1 {
        font-size: 22px;
    }

    .type-chip {
        min-height: 40px;
        padding: 6px 13px;
    }
}

.loading-wrapper {
    display: flex;
    justify-content: center;
    padding: 48px 0;
}

.search-error {
    margin-bottom: 16px;
    overflow-wrap: anywhere;
    border-radius: 14px;
}
</style>

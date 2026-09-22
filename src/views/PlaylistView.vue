<template>
    <div class="playlist-view">
        <SearchBar v-model:keyword="input" v-model:platform="currentPlatform" :platform-options="PLATFORMS"
            placeholder="请输入歌单链接或 ID" button-text="导入歌单" :loading="loading" @search="handleImport" @clear="reset" />

        <div v-if="loading" class="loading-wrapper">
            <n-spin size="medium" />
        </div>

        <div v-else-if="errorMsg" class="error-wrapper">
            <n-alert type="error" :title="errorMsg" />
        </div>

        <template v-else-if="playlist">
            <div class="playlist-info">
                <img v-if="playlist.coverUrl" :src="playlist.coverUrl" class="playlist-cover" alt="歌单封面" />
                <div class="playlist-details">
                    <div class="playlist-name">{{ playlist.name }}</div>
                    <div class="playlist-creator">创建者：{{ playlist.creator }}</div>
                    <div class="playlist-meta">歌曲数：{{ playlist.songCount }} · 播放量：{{ formatPlayCount(playlist.playCount)
                        }}</div>
                </div>
            </div>

            <div class="list-header">
                <n-checkbox :checked="isAllSelected" :indeterminate="isIndeterminate" @update:checked="toggleAll">
                    全选
                </n-checkbox>
                <span class="count-text">已选 {{ selectedIds.length }} / {{ songs.length }} 首</span>
            </div>

            <div class="song-items">
                <SongItem v-for="song in songs" :key="song.mid" :song="song" :selected="selectedIds.includes(song.mid)"
                    @toggle-select="(val) => toggleSelect(song.mid, val)" @download="(song) => downloadSingle(song)"
                    @click-artist="openRelatedArtist" @click-album="openSongAlbum" />
            </div>

            <BatchDownloadBar v-if="selectedIds.length > 0" :selectedCount="selectedIds.length"
                @batch-download="onBatchDownload" />
        </template>

        <div v-else class="empty-wrapper">
            <n-empty description="请输入歌单链接或 ID 进行导入" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onActivated, watch } from 'vue'
import { useRoute } from 'vue-router'
import { NSpin, NAlert, NEmpty, NCheckbox } from 'naive-ui'
import SearchBar from '../components/search/SearchBar.vue'
import SongItem from '../components/search/SongItem.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import { usePlaylistImport } from '../composables/usePlaylistImport'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useMusicNavigation } from '../composables/useMusicNavigation'
import { PLATFORMS, DEFAULT_PLATFORM } from '../config/platforms'
import { formatPlayCount } from '../utils/format'

const route = useRoute()
const currentPlatform = ref(DEFAULT_PLATFORM)

// 使用歌单导入 composable，并解构出响应式状态和方法
const {
    input,
    loading,
    errorMsg,
    playlist,
    songs,
    selectedIds,
    isAllSelected,
    isIndeterminate,
    toggleAll,
    toggleSelect,
    importPlaylist,
    reset,
} = usePlaylistImport()

const { downloadSingle, batchDownload } = useDownloadActions()
const { openRelatedArtist, openSongAlbum } = useMusicNavigation()

async function handleImport() {
    const term = input.value.trim()
    if (!term) return
    await importPlaylist(currentPlatform.value, term)
}

function onBatchDownload() {
    const selectedSongs = songs.value.filter(s => selectedIds.value.includes(s.mid))
    if (selectedSongs.length > 0) {
        batchDownload(selectedSongs)
    }
}

// 从路由参数加载歌单
async function loadPlaylistFromQuery() {
    if (route.path !== '/playlist') return
    const qPlatform = route.query.platform as string | undefined
    const qId = route.query.id as string | undefined
    if (qPlatform && qId) {
        // 从歌曲详情返回时复用已加载的歌单，保留勾选并避免重复请求。
        if (currentPlatform.value === qPlatform && input.value === qId && (loading.value || playlist.value)) return
        currentPlatform.value = qPlatform
        input.value = qId
        await handleImport()
    }
}

onMounted(loadPlaylistFromQuery)
onActivated(loadPlaylistFromQuery)

watch(
    () => route.query.platform + '|' + route.query.id,
    (newVal, oldVal) => {
        if (newVal && newVal !== oldVal) loadPlaylistFromQuery()
    }
)
</script>

<style scoped>
.playlist-view {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
}

/* 覆盖 SearchBar 自带的 margin-bottom，避免与父容器 gap 叠加 */
.playlist-view :deep(.search-bar) {
    margin-bottom: 0;
}

.loading-wrapper,
.error-wrapper,
.empty-wrapper {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}

.playlist-info {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 16px;
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 8px;
}

.playlist-cover {
    width: 80px;
    height: 80px;
    border-radius: 8px;
    object-fit: cover;
    flex-shrink: 0;
}

.playlist-details {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
}

.playlist-name {
    font-size: 18px;
    font-weight: 600;
    margin-bottom: 8px;
}

.playlist-creator {
    color: var(--color-text-secondary);
    font-size: 14px;
}

.playlist-meta {
    color: var(--color-text-secondary);
    font-size: 13px;
    margin-top: 4px;
}

.list-header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
}

.count-text {
    font-size: 13px;
    color: var(--color-text-secondary);
}

.song-items {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

/* 窄屏缩小信息区留白，封面保持比例，长文本自动换行 */
@media (max-width: 767px) {
    .playlist-info {
        align-items: flex-start;
        gap: 12px;
        padding: 12px;
    }

    .playlist-cover {
        width: 64px;
        height: 64px;
    }
}
</style>

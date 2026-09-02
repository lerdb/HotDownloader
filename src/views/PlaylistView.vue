<template>
    <div class="playlist-view">
        <SearchBar v-model:keyword="input" v-model:platform="currentPlatform" :platform-options="PLATFORMS"
            placeholder="请输入歌单链接或 ID" button-text="导入歌单" :loading="loading" @search="handleImport" @clear="resetPage" />

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
                    @toggle-select="(val) => toggleSelect(song.mid, val)" @download="(song) => downloadSingle(song)" />
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
import { ref, computed } from 'vue'
import { NSpin, NAlert, NEmpty, NCheckbox } from 'naive-ui'
import type { PlaylistInfo, SongInfo, PlaylistSongsResponse } from '../types'
import * as musicApi from '../api/musicApi'
import { useDownloadActions } from '../composables/useDownloadActions'
import SongItem from '../components/search/SongItem.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import SearchBar from '../components/search/SearchBar.vue'
import { PLATFORMS, DEFAULT_PLATFORM } from '../config/platforms'

const input = ref('')
const loading = ref(false)
const errorMsg = ref('')
const playlist = ref<PlaylistInfo | null>(null)
const songs = ref<SongInfo[]>([])
const selectedIds = ref<string[]>([])
const currentPlatform = ref(DEFAULT_PLATFORM)

const { downloadSingle, batchDownload } = useDownloadActions()

const isAllSelected = computed(() => songs.value.length > 0 && selectedIds.value.length === songs.value.length)
const isIndeterminate = computed(() => selectedIds.value.length > 0 && selectedIds.value.length < songs.value.length)

function toggleAll(checked: boolean) {
    selectedIds.value = checked ? songs.value.map(s => s.mid) : []
}

function toggleSelect(songMid: string, selected: boolean) {
    if (selected) {
        if (!selectedIds.value.includes(songMid)) selectedIds.value.push(songMid)
    } else {
        selectedIds.value = selectedIds.value.filter(id => id !== songMid)
    }
}

function formatPlayCount(count: number): string {
    if (count >= 10000) return (count / 10000).toFixed(1) + '万'
    return count.toString()
}

function resetPage() {
    loading.value = false
    errorMsg.value = ''
    playlist.value = null
    songs.value = []
    selectedIds.value = []
}

async function handleImport() {
    const term = input.value.trim()
    if (!term || loading.value) return

    loading.value = true
    errorMsg.value = ''
    playlist.value = null
    songs.value = []
    selectedIds.value = []

    try {
        const res: PlaylistSongsResponse = await musicApi.fetchPlaylistSongs(currentPlatform.value, term)
        playlist.value = res.playlist
        songs.value = res.songs
    } catch (e: any) {
        errorMsg.value = e?.message || String(e) || '导入歌单失败'
    } finally {
        loading.value = false
    }
}

function onBatchDownload() {
    const selectedSongs = songs.value.filter(s => selectedIds.value.includes(s.mid))
    if (selectedSongs.length > 0) {
        batchDownload(selectedSongs)
    }
}
</script>

<style scoped>
.playlist-view {
    display: flex;
    flex-direction: column;
    gap: 16px;
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
    padding: 12px;
    background-color: var(--bg-sidebar);
    border-radius: 8px;
}

.playlist-cover {
    width: 80px;
    height: 80px;
    border-radius: 8px;
    object-fit: cover;
}

.playlist-details {
    flex: 1;
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
    gap: 12px;
    margin-bottom: 8px;
}

.count-text {
    font-size: 13px;
    color: var(--color-text-secondary);
}

.song-items {
    display: flex;
    flex-direction: column;
    gap: 8px;
}
</style>
<template>
    <div class="playlist-detail">
        <n-button class="back-button" @click="emit('back')">
            {{ backLabel }}
        </n-button>

        <div v-if="loading" class="loading-wrapper">
            <n-spin size="medium" />
        </div>
        <n-alert v-else-if="error" type="error" title="加载歌单失败">
            {{ error }}
            <n-button v-if="retryable" @click="emit('retry')">重试</n-button>
        </n-alert>
        <template v-else-if="playlist">
            <div class="playlist-info">
                <img
                    v-if="playlist.coverUrl"
                    :src="playlist.coverUrl"
                    class="playlist-cover"
                    alt="歌单封面"
                />
                <div class="playlist-details">
                    <div class="playlist-name">{{ playlist.name }}</div>
                    <div v-if="playlist.creator" class="playlist-creator">
                        创建者：{{ playlist.creator }}
                    </div>
                    <div class="playlist-meta">
                        歌曲数：{{ playlist.songCount }} · 播放量：{{
                            formatPlayCount(playlist.playCount)
                        }}
                    </div>
                    <n-button
                        v-if="canMonitor"
                        class="monitor-button"
                        @click="emit('monitor')"
                    >
                        添加歌单监控
                    </n-button>
                </div>
            </div>

            <template v-if="songs.length">
                <div class="list-header">
                    <n-checkbox
                        :checked="isAllSelected"
                        :indeterminate="isIndeterminate"
                        @update:checked="
                            (checked) => emit('toggle-all', checked)
                        "
                    >
                        全选
                    </n-checkbox>
                    <span class="count-text">
                        已选 {{ selectedIds.length }} / {{ songs.length }} 首
                    </span>
                </div>
                <div class="song-items">
                    <SongItem
                        v-for="song in songs"
                        :key="song.mid"
                        :song="song"
                        :selected="selectedIds.includes(song.mid)"
                        @toggle-select="
                            (selected) =>
                                emit('toggle-select', song.mid, selected)
                        "
                        @download="(item) => emit('download', item)"
                        @click-artist="
                            (platform, artist) =>
                                emit('click-artist', platform, artist)
                        "
                        @click-album="(item) => emit('click-album', item)"
                    />
                </div>
                <BatchDownloadBar
                    v-if="selectedIds.length > 0"
                    :selected-count="selectedIds.length"
                    @batch-download="emit('batch-download')"
                />
            </template>
            <n-empty v-else description="该歌单暂无可用歌曲" />
        </template>
    </div>
</template>

<script setup lang="ts">
import { NAlert, NButton, NCheckbox, NEmpty, NSpin } from 'naive-ui'
import SongItem from '../search/SongItem.vue'
import BatchDownloadBar from '../search/BatchDownloadBar.vue'
import type { ArtistReference, PlaylistInfo, SongInfo } from '../../types'
import { formatPlayCount } from '../../utils/format'

defineProps<{
    backLabel: string
    loading: boolean
    error: string
    retryable: boolean
    playlist: PlaylistInfo | null
    songs: SongInfo[]
    selectedIds: string[]
    isAllSelected: boolean
    isIndeterminate: boolean
    canMonitor?: boolean
}>()

const emit = defineEmits<{
    (e: 'back'): void
    (e: 'monitor'): void
    (e: 'retry'): void
    (e: 'toggle-all', checked: boolean): void
    (e: 'toggle-select', songMid: string, selected: boolean): void
    (e: 'download', song: SongInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'click-album', song: SongInfo): void
    (e: 'batch-download'): void
}>()
</script>

<style scoped>
.playlist-detail {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
}

.back-button {
    align-self: flex-start;
}

.loading-wrapper {
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

.monitor-button {
    margin-top: 12px;
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

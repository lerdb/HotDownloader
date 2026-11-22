<template>
    <div class="playlist-detail">
        <n-button
            round
            secondary
            size="small"
            class="back-button"
            @click="emit('back')"
        >
            <template #icon>
                <AppIcon name="back" :size="16" />
            </template>
            {{ backLabel }}
        </n-button>

        <div v-if="loading" class="loading-wrapper">
            <n-spin size="medium" />
        </div>
        <n-alert v-else-if="error" type="error" title="加载歌单失败">
            {{ error }}
            <n-button v-if="retryable" size="small" @click="emit('retry')"
                >重试</n-button
            >
        </n-alert>
        <template v-else-if="playlist">
            <!-- SimpMusic 风格模糊封面背景 Hero 卡片 -->
            <div class="hero-card">
                <div
                    v-if="playlist.coverUrl"
                    class="hero-backdrop"
                    :style="{ backgroundImage: `url(${playlist.coverUrl})` }"
                    aria-hidden="true"
                />
                <div class="hero-scrim" aria-hidden="true" />
                <div class="hero-inner">
                    <img
                        v-if="playlist.coverUrl"
                        :src="playlist.coverUrl"
                        class="playlist-cover"
                        alt="歌单封面"
                    />
                    <div
                        v-else
                        class="playlist-cover playlist-cover-fallback"
                    >
                        <AppIcon name="playlist" :size="40" />
                    </div>
                    <div class="playlist-details">
                        <span class="hero-type-tag">歌单</span>
                        <h2 class="playlist-name">{{ playlist.name }}</h2>
                        <div v-if="playlist.creator" class="playlist-creator">
                            创建者：{{ playlist.creator }}
                        </div>
                        <div class="hero-pills">
                            <span class="meta-pill">
                                {{ playlist.songCount }} 首歌曲
                            </span>
                            <span class="meta-pill">
                                播放 {{ formatPlayCount(playlist.playCount) }}
                            </span>
                        </div>
                        <div v-if="canMonitor" class="hero-actions">
                            <n-button
                                round
                                type="primary"
                                size="small"
                                @click="emit('monitor')"
                            >
                                <template #icon>
                                    <AppIcon name="radar" :size="15" />
                                </template>
                                添加歌单监控
                            </n-button>
                        </div>
                    </div>
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
                        全选本歌单
                    </n-checkbox>
                    <span class="count-pill">
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
import AppIcon from '../common/AppIcon.vue'
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
    padding: 48px 0;
}

.hero-card {
    position: relative;
    overflow: hidden;
    border-radius: 20px;
    border: 1px solid var(--border-color);
    background: var(--bg-sidebar);
    box-shadow: var(--shadow-md);
}

.hero-backdrop {
    position: absolute;
    inset: -24px;
    background-size: cover;
    background-position: center;
    filter: blur(36px) saturate(140%);
    opacity: 0.25;
}

.hero-scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(
        135deg,
        var(--bg-glass) 0%,
        var(--bg-glass-strong) 100%
    );
}

.hero-inner {
    position: relative;
    z-index: 1;
    display: flex;
    gap: 20px;
    align-items: center;
    padding: 22px;
}

.playlist-cover {
    width: 120px;
    height: 120px;
    border-radius: 14px;
    object-fit: cover;
    flex-shrink: 0;
    box-shadow: var(--shadow-md);
}

.playlist-cover-fallback {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.playlist-details {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
}

.hero-type-tag {
    display: inline-block;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-size: 11px;
    font-weight: 600;
    margin-bottom: 6px;
}

.playlist-name {
    font-size: 22px;
    font-weight: 700;
    line-height: 1.3;
    margin: 0 0 6px;
}

.playlist-creator {
    color: var(--color-text-secondary);
    font-size: 13px;
}

.hero-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
}

.meta-pill {
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
    font-size: 12px;
    font-weight: 500;
}

.hero-actions {
    margin-top: 12px;
}

.list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px;
    padding: 8px 14px;
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 12px;
}

.count-pill {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-secondary);
    padding: 2px 10px;
    border-radius: 999px;
    background: var(--bg-surface-subtle);
}

.song-items {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

@media (max-width: 767px) {
    .hero-inner {
        align-items: flex-start;
        gap: 14px;
        padding: 16px;
    }

    .playlist-cover {
        width: 84px;
        height: 84px;
        border-radius: 12px;
    }

    .playlist-name {
        font-size: 18px;
    }
}
</style>

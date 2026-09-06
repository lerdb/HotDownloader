<template>
    <div class="playlist-search-result">
        <template v-if="playlists.length > 0">
            <div class="playlist-card-list">
                <div v-for="pl in playlists" :key="pl.id" class="playlist-card" @click="$emit('click-playlist', pl)">
                    <img v-if="pl.coverUrl" :src="pl.coverUrl" class="playlist-card-cover" alt="歌单封面" />
                    <div class="playlist-card-info">
                        <div class="playlist-card-name">{{ pl.name }}</div>
                        <div class="playlist-card-creator">{{ pl.creator }}</div>
                        <div class="playlist-card-meta">{{ pl.songCount }} 首 · {{ formatPlayCount(pl.playCount) }}</div>
                    </div>
                </div>
            </div>
            <LoadMoreButton v-if="hasMore" :loading="loadingMore" :disabled="loadingMore" @click="$emit('load-more')" />
        </template>
        <div v-else class="empty-result">
            <n-empty description="未找到相关歌单" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { NEmpty } from 'naive-ui'
import type { PlaylistSearchItem } from '../../types'
import LoadMoreButton from './LoadMoreButton.vue'

defineProps<{
    playlists: PlaylistSearchItem[]
    hasMore: boolean
    loadingMore: boolean
}>()

defineEmits<{
    (e: 'click-playlist', playlist: PlaylistSearchItem): void
    (e: 'load-more'): void
}>()

function formatPlayCount(count: number): string {
    if (count >= 10000) return (count / 10000).toFixed(1) + '万'
    return count.toString()
}
</script>

<style scoped>
.playlist-card-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.playlist-card {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 12px;
    background-color: var(--bg-sidebar);
    border-radius: 8px;
    cursor: pointer;
    transition: background-color 0.2s;
}

.playlist-card:hover {
    background-color: var(--bg-hover, rgba(0, 0, 0, 0.05));
}

.playlist-card-cover {
    width: 60px;
    height: 60px;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
}

.playlist-card-info {
    flex: 1;
    min-width: 0;
}

.playlist-card-name {
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.playlist-card-creator {
    color: var(--color-text-secondary);
    font-size: 13px;
    margin-top: 2px;
}

.playlist-card-meta {
    color: var(--color-text-secondary);
    font-size: 12px;
    margin-top: 2px;
}

.empty-result {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}
</style>
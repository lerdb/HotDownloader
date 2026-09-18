<template>
    <div class="artist-search-result">
        <template v-if="artists.length > 0">
            <div class="artist-card-list">
                <button type="button" v-for="pl in artists" :key="pl.id" class="artist-card" @click="$emit('click-artist', pl)">
                    <img v-if="pl.coverUrl" :src="pl.coverUrl" class="artist-card-cover" alt="歌手封面" />
                    <div class="artist-card-info">
                        <div class="artist-card-name">{{ pl.name }}</div>
                        <div class="artist-card-creator">{{ [pl.alias, pl.region].filter(Boolean).join(' · ') }}</div>
                        <div class="artist-card-meta">{{ pl.songCount }} 首歌曲 · {{ pl.albumCount }} 张专辑</div>
                    </div>
                </button>
            </div>
            <LoadMoreButton v-if="hasMore" :loading="loadingMore" :disabled="loadingMore" @click="$emit('load-more')" />
        </template>
        <div v-else class="empty-result">
            <n-empty description="未找到相关歌手" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { NEmpty } from 'naive-ui'
import type { ArtistInfo } from '../../types'
import LoadMoreButton from './LoadMoreButton.vue'


defineProps<{
    artists: ArtistInfo[]
    hasMore: boolean
    loadingMore: boolean
}>()

defineEmits<{
    (e: 'click-artist', artist: ArtistInfo): void
    (e: 'load-more'): void
}>()
</script>

<style scoped>
.artist-card-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.artist-card {
    width: 100%;
    text-align: left;
    color: inherit;
    font: inherit;
    display: flex;
    gap: 12px;
    align-items: center;
    min-width: 0;
    padding: 12px;
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    cursor: pointer;
    transition: border-color 0.2s;
}

.artist-card:hover {
    border-color: var(--color-text-secondary);
}

.artist-card-cover {
    width: 60px;
    height: 60px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
}

.artist-card-info {
    flex: 1;
    min-width: 0;
}

.artist-card-name {
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.artist-card-creator {
    overflow-wrap: anywhere;
    color: var(--color-text-secondary);
    font-size: 13px;
    margin-top: 2px;
}

.artist-card-meta {
    overflow-wrap: anywhere;
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

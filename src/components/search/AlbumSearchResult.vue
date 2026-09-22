<template>
    <div class="album-search-result">
        <template v-if="albums.length > 0">
            <div class="album-card-list">
                <div v-for="album in albums" :key="album.id" class="album-card" @click="$emit('click-album', album)">
                    <img v-if="album.coverUrl" :src="album.coverUrl" class="album-card-cover" alt="专辑封面" />
                    <div class="album-card-info">
                        <div class="album-card-name">
                            <n-button text @click.stop="$emit('click-album', album)">{{ album.name }}</n-button>
                        </div>
                        <div class="album-card-creator">
                            <ArtistNames :platform="platform" :artists="album.artists" :fallback="album.artist"
                                @click-artist="(platform, artist) => $emit('click-artist', platform, artist)" />
                        </div>
                        <div class="album-card-meta">
                            {{ album.songCount }} 首<span v-if="album.publishDate"> · {{ album.publishDate }}</span>
                        </div>
                    </div>
                </div>
            </div>
            <LoadMoreButton v-if="hasMore" :loading="loadingMore" :disabled="loadingMore" @click="$emit('load-more')" />
        </template>
        <div v-else class="empty-result">
            <n-empty description="未找到相关专辑" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { NButton, NEmpty } from 'naive-ui'
import type { AlbumInfo, ArtistReference } from '../../types'
import LoadMoreButton from './LoadMoreButton.vue'
import ArtistNames from './ArtistNames.vue'

defineProps<{
    platform: string
    albums: AlbumInfo[]
    hasMore: boolean
    loadingMore: boolean
}>()

defineEmits<{
    (e: 'click-album', album: AlbumInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'load-more'): void
}>()
</script>

<style scoped>
.album-card-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.album-card {
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

.album-card:hover {
    border-color: var(--color-text-secondary);
}

.album-card-cover {
    width: 60px;
    height: 60px;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
}

.album-card-info {
    flex: 1;
    min-width: 0;
}

.album-card-name {
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.album-card-creator {
    overflow-wrap: anywhere;
    color: var(--color-text-secondary);
    font-size: 13px;
    margin-top: 2px;
}

.album-card-name .n-button {
    font: inherit;
}

.album-card-meta {
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

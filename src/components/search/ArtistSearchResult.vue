<template>
    <div class="artist-search-result">
        <template v-if="artists.length > 0">
            <div class="artist-card-list">
                <button
                    type="button"
                    v-for="pl in artists"
                    :key="pl.id"
                    class="artist-card"
                    @click="$emit('click-artist', pl)"
                >
                    <img
                        v-if="pl.coverUrl"
                        :src="pl.coverUrl"
                        class="artist-card-cover"
                        alt="歌手封面"
                        loading="lazy"
                    />
                    <div v-else class="artist-card-cover artist-cover-fallback">
                        <AppIcon name="artist" :size="24" />
                    </div>
                    <div class="artist-card-info">
                        <div class="artist-card-name">{{ pl.name }}</div>
                        <div
                            v-if="pl.alias || pl.region"
                            class="artist-card-creator"
                        >
                            {{
                                [pl.alias, pl.region]
                                    .filter(Boolean)
                                    .join(' · ')
                            }}
                        </div>
                        <div class="artist-card-meta">
                            {{ pl.songCount }} 首歌曲 ·
                            {{ pl.albumCount }} 张专辑
                        </div>
                    </div>
                    <AppIcon
                        name="chevron-right"
                        :size="16"
                        class="card-chevron"
                    />
                </button>
            </div>
            <LoadMoreButton
                v-if="hasMore"
                :loading="loadingMore"
                :disabled="loadingMore"
                @click="$emit('load-more')"
            />
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
import AppIcon from '../common/AppIcon.vue'

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
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
}

.artist-card {
    width: 100%;
    text-align: left;
    color: inherit;
    font: inherit;
    display: flex;
    gap: 14px;
    align-items: center;
    min-width: 0;
    padding: 14px;
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 16px;
    cursor: pointer;
    transition:
        border-color 0.18s ease,
        background-color 0.18s ease,
        transform 0.15s ease,
        box-shadow 0.18s ease;
}

.artist-card:hover {
    border-color: var(--color-primary);
    background-color: var(--bg-elevated);
    transform: translateY(-1px);
    box-shadow: var(--shadow-sm);
}

.artist-card-cover {
    width: 60px;
    height: 60px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
    box-shadow: var(--shadow-sm);
}

.artist-cover-fallback {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
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
    font-size: 12px;
    margin-top: 3px;
}

.artist-card-meta {
    overflow-wrap: anywhere;
    color: var(--color-text-secondary);
    font-size: 12px;
    margin-top: 3px;
}

.card-chevron {
    color: var(--color-text-secondary);
    opacity: 0.6;
    flex-shrink: 0;
}

.empty-result {
    display: flex;
    justify-content: center;
    padding: 48px 0;
}

@media (max-width: 600px) {
    .artist-card-list {
        grid-template-columns: 1fr;
    }
}
</style>

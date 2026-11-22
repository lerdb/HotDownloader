<template>
    <div class="search-result-list">
        <template v-if="songs.length > 0">
            <div class="list-header">
                <n-checkbox
                    :checked="isAllSelected"
                    :indeterminate="isIndeterminate"
                    @update:checked="toggleAll"
                >
                    全选本页
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
                    @toggle-select="(val) => toggleSelect(song.mid, val)"
                    @download="(song) => $emit('download', song)"
                    @click-artist="
                        (platform, artist) =>
                            $emit('click-artist', platform, artist)
                    "
                    @click-album="(song) => $emit('click-album', song)"
                />
            </div>

            <LoadMoreButton
                v-if="hasMore"
                :loading="loadingMore"
                :disabled="loadingMore"
                @click="$emit('load-more')"
            />
        </template>

        <div v-else class="empty-result">
            <n-empty description="暂无搜索结果" />
            <div class="retry-wrapper">
                <n-button type="primary" round @click="$emit('retry')"
                    >重试</n-button
                >
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { NCheckbox, NEmpty, NButton } from 'naive-ui'
import type { ArtistReference, SongInfo } from '../../types'
import SongItem from './SongItem.vue'
import LoadMoreButton from './LoadMoreButton.vue'

const props = withDefaults(
    defineProps<{
        songs: SongInfo[]
        selectedIds: string[]
        hasMore?: boolean
        loadingMore?: boolean
    }>(),
    {
        hasMore: false,
        loadingMore: false,
    },
)

const emit = defineEmits<{
    (e: 'update:selectedIds', ids: string[]): void
    (e: 'download', song: SongInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'click-album', song: SongInfo): void
    (e: 'retry'): void
    (e: 'load-more'): void
}>()

const isAllSelected = computed(
    () =>
        props.songs.length > 0 &&
        props.selectedIds.length === props.songs.length,
)

const isIndeterminate = computed(
    () =>
        props.selectedIds.length > 0 &&
        props.selectedIds.length < props.songs.length,
)

function toggleAll(checked: boolean) {
    if (checked) {
        emit(
            'update:selectedIds',
            props.songs.map((s) => s.mid),
        )
    } else {
        emit('update:selectedIds', [])
    }
}

function toggleSelect(songMid: string, selected: boolean) {
    let newIds: string[]
    if (selected) {
        newIds = [...props.selectedIds, songMid]
    } else {
        newIds = props.selectedIds.filter((id) => id !== songMid)
    }
    emit('update:selectedIds', newIds)
}
</script>

<style scoped>
.list-header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 14px;
    margin-bottom: 12px;
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

.empty-result {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 48px 0;
}

.retry-wrapper {
    margin-top: 16px;
    text-align: center;
}
</style>

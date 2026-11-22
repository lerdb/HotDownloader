<template>
    <div class="search-suggestions" v-if="hasAny">
        <!-- 单曲 -->
        <div v-if="data.song.length > 0" class="suggest-group">
            <div class="group-title">
                <AppIcon name="music" :size="14" />
                <span>单曲</span>
            </div>
            <div
                v-for="(item, index) in data.song"
                :key="item.mid ?? item.id ?? `song-${index}`"
                class="suggest-item"
                @click="handleSelect(item, 'song')"
            >
                <span class="item-name">{{ item.name }}</span>
                <span v-if="item.singer" class="item-singer"
                    >· {{ item.singer }}</span
                >
            </div>
        </div>

        <!-- 歌手 -->
        <div v-if="data.singer.length > 0" class="suggest-group">
            <div class="group-title">
                <AppIcon name="artist" :size="14" />
                <span>歌手</span>
            </div>
            <div
                v-for="(item, index) in data.singer"
                :key="item.mid ?? item.id ?? `singer-${index}`"
                class="suggest-item"
                @click="handleSelect(item, 'singer')"
            >
                <span class="item-name">{{ item.name }}</span>
            </div>
        </div>

        <!-- 专辑 -->
        <div v-if="data.album.length > 0" class="suggest-group">
            <div class="group-title">
                <AppIcon name="disc" :size="14" />
                <span>专辑</span>
            </div>
            <div
                v-for="(item, index) in data.album"
                :key="item.mid ?? item.id ?? `album-${index}`"
                class="suggest-item"
                @click="handleSelect(item, 'album')"
            >
                <span class="item-name">{{ item.name }}</span>
                <span v-if="item.singer" class="item-singer"
                    >· {{ item.singer }}</span
                >
            </div>
        </div>

        <!-- MV -->
        <div v-if="data.mv.length > 0" class="suggest-group">
            <div class="group-title">
                <AppIcon name="play" :size="14" />
                <span>MV</span>
            </div>
            <div
                v-for="(item, index) in data.mv"
                :key="item.vid ?? item.mid ?? item.id ?? `mv-${index}`"
                class="suggest-item"
                @click="handleSelect(item, 'mv')"
            >
                <span class="item-name">{{ item.name }}</span>
                <span v-if="item.singer" class="item-singer"
                    >· {{ item.singer }}</span
                >
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { SearchSuggestionData, SearchSuggestionItem } from '../../types'
import AppIcon from '../common/AppIcon.vue'

const props = defineProps<{
    data: SearchSuggestionData
}>()

const emit = defineEmits<{
    (e: 'select', keyword: string, type: keyof SearchSuggestionData): void
}>()

function handleSelect(
    item: SearchSuggestionItem,
    type: keyof SearchSuggestionData,
) {
    if (item.name) {
        emit('select', item.name, type)
    }
}

const hasAny = computed(
    () =>
        props.data.song.length > 0 ||
        props.data.singer.length > 0 ||
        props.data.album.length > 0 ||
        props.data.mv.length > 0,
)
</script>

<style scoped>
.search-suggestions {
    margin-top: 8px;
    padding: 14px;
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-md);
}

.suggest-group {
    margin-bottom: 12px;
}

.suggest-group:last-child {
    margin-bottom: 0;
}

.group-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--color-primary);
    margin-bottom: 6px;
    padding-left: 8px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
}

.suggest-item {
    min-height: 44px;
    box-sizing: border-box;
    padding: 8px 12px;
    border-radius: 10px;
    cursor: pointer;
    transition: background-color 0.15s ease;
    display: flex;
    align-items: baseline;
    gap: 6px;
}

.suggest-item:hover {
    background: var(--bg-surface-subtle);
}

.item-name {
    min-width: 0;
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.item-singer {
    min-width: 0;
    max-width: 50%;
    font-size: 12px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
</style>

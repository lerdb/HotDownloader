<template>
    <div class="song-item" :class="{ 'is-selected': selected }">
        <n-checkbox :checked="selected" @update:checked="$emit('toggleSelect', $event)" />
        <div class="cover-wrapper">
            <img v-if="coverUrl" :src="coverUrl" class="cover" alt="封面" loading="lazy" />
            <div v-else-if="coverLoading" class="cover placeholder" />
            <div v-else class="cover placeholder default" />
        </div>
        <div class="info">
            <div class="title">{{ song.title }}</div>
            <div class="subtitle">
                <ArtistNames :platform="song.platform" :artists="song.artists" :fallback="song.artist"
                    @click-artist="(platform, artist) => $emit('click-artist', platform, artist)" />
            </div>
            <div v-if="song.album" class="subtitle">
                <n-button v-if="albumId" text size="small" class="album-link" @click.stop="$emit('click-album', song)">
                    {{ song.album }}
                </n-button>
                <span v-else>{{ song.album }}</span>
            </div>
            <div class="quality-tags">
                <n-tag v-for="q in sortedQualities.slice(0, 4)" :key="q.quality" size="tiny" :bordered="false"
                    type="info">
                    {{ q.quality }}
                </n-tag>
                <n-tag v-if="sortedQualities.length > 4" size="tiny" :bordered="false" type="info">
                    +{{ sortedQualities.length - 4 }}
                </n-tag>
            </div>
        </div>
        <n-button size="small" class="download-btn" @click="$emit('download', song)">
            下载
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { NCheckbox, NButton, NTag } from 'naive-ui'
import type { ArtistReference, SongInfo } from '../../types'
import { ALL_QUALITY_ORDER } from '../../types'
import { fetchCover } from '../../api/musicApi'
import ArtistNames from './ArtistNames.vue'
import { getMusicEntityId } from '../../utils/music'

const props = defineProps<{
    song: SongInfo
    selected: boolean
}>()

defineEmits<{
    (e: 'toggleSelect', selected: boolean): void
    (e: 'download', song: SongInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'click-album', song: SongInfo): void
}>()

const albumId = computed(() => getMusicEntityId(props.song.platform, props.song.albumId, props.song.albumMid))

// 按品质从高到低排序
const sortedQualities = computed(() => {
    return [...props.song.qualities].sort((a, b) => {
        const ia = ALL_QUALITY_ORDER.indexOf(a.quality)
        const ib = ALL_QUALITY_ORDER.indexOf(b.quality)
        // 未知品质放在末尾
        const idxA = ia === -1 ? -1 : ia
        const idxB = ib === -1 ? -1 : ib
        return idxB - idxA  // 降序
    })
})

// 优先展示歌曲自带的封面，缺少地址时按需请求。
const coverUrl = ref<string>('')
const coverLoading = ref(false)

async function loadCoverIfNeeded() {
    // 已有 URL 直接使用
    if (props.song.coverUrl) {
        coverUrl.value = props.song.coverUrl
        return
    }
    // 酷我场景下按需加载
    if (!props.song.id) return
    coverLoading.value = true
    try {
        const url = await fetchCover('kuwo', props.song.id)
        // 检查组件是否已被卸载（song prop 改变）
        if (props.song.id === props.song.id) {
            coverUrl.value = url
        }
    } catch {
        // 加载失败保持占位
    } finally {
        coverLoading.value = false
    }
}

onMounted(() => {
    loadCoverIfNeeded()
})

// 切换 song prop 时（如列表项重用）重新加载
watch(() => props.song.id, () => {
    coverUrl.value = props.song.coverUrl
    loadCoverIfNeeded()
})
</script>

<style scoped>
.song-item {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 12px;
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 8px;
}

.song-item.is-selected {
    border-color: var(--color-text-secondary);
}

.cover-wrapper {
    width: 48px;
    height: 48px;
    flex-shrink: 0;
}

.cover {
    width: 48px;
    height: 48px;
    border-radius: 6px;
    object-fit: cover;
    display: block;
}

.cover.placeholder {
    background-color: var(--border-color);
}

.cover.placeholder.default {
    background-color: var(--bg-body);
}

.info {
    flex: 1;
    min-width: 0;
    overflow: hidden;
}

.title {
    font-size: 15px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-text);
    line-height: 1.5;
}

.subtitle {
    margin-top: 2px;
    font-size: 13px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.album-link {
    font: inherit;
    vertical-align: baseline;
}

.quality-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
}

.download-btn {
    flex-shrink: 0;
}

/* 手机保留封面和操作入口，长歌名在剩余空间内省略 */
@media (max-width: 767px) {
    .song-item {
        gap: 8px;
        padding: 10px 8px;
    }

    .download-btn {
        min-height: 44px;
    }
}
</style>

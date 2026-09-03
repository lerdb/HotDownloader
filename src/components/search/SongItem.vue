<template>
    <div class="song-item">
        <n-checkbox :checked="selected" @update:checked="$emit('toggleSelect', $event)" />
        <div class="cover-wrapper">
            <img v-if="coverUrl" :src="coverUrl" class="cover" alt="封面" loading="lazy" />
            <div v-else-if="coverLoading" class="cover placeholder" />
            <div v-else class="cover placeholder default" />
        </div>
        <div class="info">
            <div class="title">{{ song.title }}</div>
            <div class="subtitle">{{ song.artist }} · {{ song.album }}</div>
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
        <n-button size="small" @click="$emit('download', song)">
            下载
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { NCheckbox, NButton, NTag } from 'naive-ui'
import type { SongInfo } from '../../types'
import { ALL_QUALITY_ORDER } from '../../types'
import { fetchCover } from '../../api/musicApi'

const props = defineProps<{
    song: SongInfo
    selected: boolean
}>()

defineEmits<{
    (e: 'toggleSelect', selected: boolean): void
    (e: 'download', song: SongInfo): void
}>()

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

// 封面 URL 懒加载：QQ 音乐自带 coverUrl；酷我需要通过后端接口按需获取
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
    padding: 8px;
    border: 1px solid var(--n-border-color, #eee);
    border-radius: 8px;
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
    background-color: var(--n-color-hover, rgba(0, 0, 0, 0.04));
}

.cover.placeholder.default {
    background-color: var(--bg-body, #f5f5f5);
}

.info {
    flex: 1;
    overflow: hidden;
}

.title {
    font-size: 15px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-text);
}

.subtitle {
    font-size: 13px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.quality-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
}
</style>
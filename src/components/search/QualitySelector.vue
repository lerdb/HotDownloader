<template>
    <div class="quality-selector">
        <n-button v-for="q in sortedQualities" :key="q.quality" :type="selected === q.quality ? 'primary' : 'default'"
            block class="quality-btn" @click="selected = q.quality">
            <div class="quality-name">{{ q.quality }}</div>
            <div class="quality-size">{{ formatSize(q.size) }}</div>
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { NButton } from 'naive-ui'
import type { QualityItem } from '../../types'
import { ALL_QUALITY_ORDER } from '../../types'

const props = defineProps<{
    qualities: QualityItem[]
}>()

// 从高到低排序
const sortedQualities = computed(() =>
    [...props.qualities].sort(
        (a, b) =>
            ALL_QUALITY_ORDER.indexOf(b.quality) - ALL_QUALITY_ORDER.indexOf(a.quality)
    )
)

// 默认选中最高可用品质（排序后的第一个）
const getDefaultQuality = (): string => {
    const sorted = [...props.qualities].sort(
        (a, b) =>
            ALL_QUALITY_ORDER.indexOf(b.quality) - ALL_QUALITY_ORDER.indexOf(a.quality)
    )
    return sorted[0]?.quality ?? ''
}

const selected = ref(getDefaultQuality())

function formatSize(bytes: number): string {
    return `${(bytes / 1048576).toFixed(2)} MB`
}

defineExpose({ selected })
</script>

<style scoped>
.quality-selector {
    max-height: 320px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.quality-btn {
    flex-shrink: 0;
    height: auto;
    padding: 10px 14px;
    border-radius: 8px;
    transition: all 0.2s ease;
}

.quality-btn :deep(.n-button__content) {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
}

.quality-name {
    white-space: normal;
    overflow-wrap: anywhere;
    font-weight: 600;
    font-size: 15px;
    margin-bottom: 4px;
}

.quality-size {
    font-size: 12px;
    color: var(--color-text-secondary);
    line-height: 1.4;
}
</style>

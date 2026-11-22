<template>
    <div v-if="selectedCount > 0" class="batch-bar">
        <span class="selected-text">
            <span class="selected-badge">{{ selectedCount }}</span>
            首歌曲已选中
        </span>
        <n-button
            type="primary"
            round
            class="batch-download-btn"
            @click="$emit('batch-download')"
        >
            <template #icon>
                <AppIcon name="download" :size="16" />
            </template>
            批量下载
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { NButton } from 'naive-ui'
import AppIcon from '../common/AppIcon.vue'

defineProps<{
    selectedCount: number
}>()

defineEmits<{
    (e: 'batch-download'): void
}>()
</script>

<style scoped>
.batch-bar {
    position: sticky;
    bottom: 8px;
    z-index: 2;
    background: var(--bg-glass-strong);
    backdrop-filter: blur(20px) saturate(160%);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--border-color-strong);
    border-radius: 16px;
    margin-top: 14px;
    padding: 12px 18px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
    align-items: center;
    justify-content: space-between;
    box-shadow: var(--shadow-md);
}

.selected-text {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text);
}

.selected-badge {
    min-width: 24px;
    height: 24px;
    padding: 0 8px;
    border-radius: 999px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-size: 13px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-variant-numeric: tabular-nums;
}

.batch-download-btn {
    flex-shrink: 0;
}

@media (max-width: 767px) {
    .batch-bar {
        padding: 10px 14px;
    }

    .batch-download-btn {
        min-height: 44px;
    }
}
</style>

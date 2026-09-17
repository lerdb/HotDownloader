<template>
    <div v-if="selectedCount > 0" class="batch-bar">
        <span class="selected-text">已选择 {{ selectedCount }} 首</span>
        <n-button type="primary" class="batch-download-btn" @click="$emit('batch-download')">
            批量下载
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { NButton } from 'naive-ui'

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
    bottom: 0;
    z-index: 1;
    background-color: var(--bg-bottom);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    margin-top: 12px;
    padding: 12px 16px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
    align-items: center;
    justify-content: space-between;
    box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.06);
}

.selected-text {
    font-weight: 500;
    color: var(--color-text);
}

.batch-download-btn {
    flex-shrink: 0;
}

@media (max-width: 767px) {
    .batch-bar {
        padding: 8px 12px;
    }

    .batch-download-btn {
        min-height: 44px;
    }
}
</style>

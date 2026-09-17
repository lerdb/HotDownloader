<template>
    <div v-if="!loading && keywords.length > 0" class="hot-keywords">
        <div class="hot-header">热搜推荐</div>
        <div class="hot-list">
            <n-tag v-for="word in keywords" :key="word" class="hot-tag" size="medium" @click="$emit('select', word)">
                {{ word }}
            </n-tag>
        </div>
    </div>
    <div v-else-if="loading" class="hot-loading">
        <n-spin size="small" />
    </div>
</template>

<script setup lang="ts">
import { NTag, NSpin } from 'naive-ui'

defineProps<{
    keywords: string[]
    loading: boolean
}>()

defineEmits<{
    (e: 'select', word: string): void
}>()
</script>

<style scoped>
.hot-keywords {
    margin-top: 16px;
}

.hot-header {
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text-secondary);
    margin-bottom: 8px;
}

.hot-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.hot-tag {
    cursor: pointer;
    max-width: 100%;
    transition: opacity 0.2s;
}

.hot-tag :deep(.n-tag__content) {
    min-width: 0;
    white-space: normal;
    overflow-wrap: anywhere;
}

.hot-tag.n-tag {
    height: auto;
    min-height: 32px;
    padding-top: 4px;
    padding-bottom: 4px;
}

.hot-tag:hover {
    opacity: 0.8;
}

.hot-loading {
    display: flex;
    justify-content: center;
    padding: 12px 0;
}
</style>

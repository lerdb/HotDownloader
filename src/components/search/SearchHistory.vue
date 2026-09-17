<template>
    <div class="search-history" v-if="history.length > 0">
        <div class="history-header">
            <span>搜索历史</span>
            <n-button text type="primary" size="small" @click="$emit('clear')">
                清除历史
            </n-button>
        </div>
        <div class="history-tags">
            <n-tag v-for="item in history" :key="item" closable @close="$emit('remove', item)"
                @click="$emit('select', item)" class="history-tag">
                {{ item }}
            </n-tag>
        </div>
    </div>
</template>

<script setup lang="ts">
import { NTag, NButton } from 'naive-ui'

defineProps<{
    history: string[]
}>()

defineEmits<{
    (e: 'select', keyword: string): void
    (e: 'remove', keyword: string): void
    (e: 'clear'): void
}>()
</script>

<style scoped>
.search-history {
    margin-bottom: 16px;
}

.history-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
}

.history-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.history-tag {
    cursor: pointer;
    max-width: 100%;
}

/* 长关键词换行显示，保留标签末尾的删除入口 */
.history-tag :deep(.n-tag__content) {
    min-width: 0;
    white-space: normal;
    overflow-wrap: anywhere;
}

.history-tag.n-tag {
    height: auto;
    min-height: 32px;
    padding-top: 4px;
    padding-bottom: 4px;
}
</style>

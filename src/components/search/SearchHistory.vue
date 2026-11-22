<template>
    <section class="search-history card-section" v-if="history.length > 0">
        <div class="history-header">
            <div class="section-heading">
                <AppIcon name="history" :size="16" class="heading-icon" />
                <span>最近搜索</span>
            </div>
            <n-button text type="primary" size="small" @click="$emit('clear')">
                清除历史
            </n-button>
        </div>
        <div class="history-tags">
            <n-tag
                v-for="item in history"
                :key="item"
                closable
                round
                :bordered="false"
                @close="$emit('remove', item)"
                @click="$emit('select', item)"
                class="history-tag"
            >
                {{ item }}
            </n-tag>
        </div>
    </section>
</template>

<script setup lang="ts">
import { NTag, NButton } from 'naive-ui'
import AppIcon from '../common/AppIcon.vue'

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
.card-section {
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-card);
    padding: 16px 18px;
    margin-bottom: 14px;
    box-shadow: var(--shadow-sm);
}

.history-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
}

.section-heading {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
}

.heading-icon {
    color: var(--color-primary);
}

.history-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.history-tag {
    cursor: pointer;
    max-width: 100%;
    background: var(--bg-surface-subtle) !important;
    transition:
        transform 0.15s ease,
        background-color 0.18s ease;
}

.history-tag:hover {
    transform: translateY(-1px);
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
    padding: 5px 12px;
}
</style>

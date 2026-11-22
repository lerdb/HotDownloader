<template>
    <div class="task-tabs" role="tablist" aria-label="任务状态筛选">
        <button
            v-for="tab in tabs"
            :key="tab.name"
            type="button"
            role="tab"
            :aria-selected="activeTab === tab.name"
            class="task-tab-pill"
            :class="{ 'is-active': activeTab === tab.name }"
            @click="$emit('update:activeTab', tab.name)"
        >
            <span>{{ tab.label }}</span>
            <span class="tab-count">{{ tab.count }}</span>
        </button>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export interface TabCounts {
    total: number
    waiting: number
    downloading: number
    paused: number
    completed: number
    interrupted: number
    error: number
}

const props = defineProps<{
    activeTab: string
    counts: TabCounts
}>()

defineEmits<{
    (e: 'update:activeTab', value: string): void
}>()

const tabs = computed(() => [
    { name: 'all', label: '全部', count: props.counts.total },
    { name: 'waiting', label: '等待中', count: props.counts.waiting },
    { name: 'downloading', label: '下载中', count: props.counts.downloading },
    { name: 'paused', label: '暂停', count: props.counts.paused },
    { name: 'completed', label: '已完成', count: props.counts.completed },
    { name: 'interrupted', label: '已中断', count: props.counts.interrupted },
    { name: 'error', label: '错误', count: props.counts.error },
])
</script>

<style scoped>
.task-tabs {
    display: flex;
    align-items: center;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 6px;
    margin-bottom: 12px;
    scrollbar-width: none;
}

.task-tabs::-webkit-scrollbar {
    display: none;
}

.task-tab-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    min-height: 36px;
    padding: 6px 13px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--bg-sidebar);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition:
        background-color 0.18s ease,
        color 0.18s ease,
        border-color 0.18s ease;
}

.task-tab-pill:hover {
    color: var(--color-text);
    border-color: var(--border-color-strong);
}

.task-tab-pill.is-active {
    background: var(--color-primary-soft);
    color: var(--color-primary);
    border-color: var(--color-primary);
    font-weight: 600;
}

.tab-count {
    font-size: 11px;
    font-weight: 600;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
}

.task-tab-pill.is-active .tab-count {
    background: var(--color-primary);
    color: #ffffff;
}

:root[data-theme='dark'] .task-tab-pill.is-active .tab-count {
    color: #0b1210;
}
</style>

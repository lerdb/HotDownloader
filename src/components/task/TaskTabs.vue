<template>
    <n-tabs :value="activeTab" @update:value="$emit('update:activeTab', $event)"
        :type="isNarrow ? 'line' : 'segment'" size="medium">
        <n-tab-pane name="all" :tab="`全部 (${counts.total})`" />
        <n-tab-pane name="waiting" :tab="`等待中 (${counts.waiting})`" />
        <n-tab-pane name="downloading" :tab="`下载中 (${counts.downloading})`" />
        <n-tab-pane name="paused" :tab="`暂停 (${counts.paused})`" />
        <n-tab-pane name="completed" :tab="`已完成 (${counts.completed})`" />
        <n-tab-pane name="error" :tab="`错误 (${counts.error})`" />
    </n-tabs>
</template>

<script setup lang="ts">
import { NTabs, NTabPane } from 'naive-ui'
import { useNarrowLayout } from '../../composables/useNarrowLayout'

const isNarrow = useNarrowLayout()

export interface TabCounts {
    total: number
    waiting: number
    downloading: number
    paused: number
    completed: number
    error: number
}

defineProps<{
    activeTab: string
    counts: TabCounts
}>()

defineEmits<{
    (e: 'update:activeTab', value: string): void
}>()
</script>

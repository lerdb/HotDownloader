<template>
    <n-modal
        v-model:show="showHistory"
        preset="card"
        :title="`${historyName} · 检查历史`"
        class="monitor-modal"
    >
        <p class="muted">
            保留最近 100
            轮，按时间倒序排列。新增为歌单成员相对上次快照的增量；关联为本轮自动匹配本地文件；入队包含成功提交的新任务与重试，不代表下载完成。同一歌曲在参与本轮的多个歌单中分别计数。
        </p>
        <n-alert v-if="historyError" type="error">{{ historyError }}</n-alert>
        <n-button :loading="historyLoading" @click="emit('refresh')"
            >刷新历史</n-button
        >
        <n-empty
            v-if="!historyLoading && !history.length"
            description="暂无检查历史"
        />
        <article
            v-for="(record, index) in history.slice(
                (historyPage - 1) * 10,
                historyPage * 10,
            )"
            :key="index"
            class="song-row"
        >
            <strong
                >{{ time(record.startedAt) }} ·
                {{
                    {
                        manual: '立即检查',
                        scheduled: '定时检查',
                        backfill: '补齐或重试',
                    }[record.trigger]
                }}</strong
            >
            <p>
                {{ roundLabel(record.status) }} · 结束：{{
                    time(
                        record.finishedAt,
                        record.status === 'running' ? '进行中' : '未记录',
                    )
                }}
            </p>
            <p>
                新增 {{ record.added }} · 关联 {{ record.linked }} · 入队
                {{ record.enqueued }} · 派发失败 {{ record.failed }}
            </p>
            <p class="muted">{{ record.message }}</p>
        </article>
        <n-pagination
            v-model:page="historyPage"
            :page-size="10"
            :item-count="history.length"
        />
    </n-modal>
</template>

<script setup lang="ts">
import { NAlert, NButton, NEmpty, NModal, NPagination } from 'naive-ui'
import type { CheckRecord } from '../../api/monitorApi'
import { time, roundLabel } from './presentation'
const showHistory = defineModel<boolean>('showHistory', { required: true })
const historyPage = defineModel<number>('historyPage', { required: true })
defineProps<{
    historyName: string
    historyError: string
    historyLoading: boolean
    history: CheckRecord[]
}>()
const emit = defineEmits<{ refresh: [] }>()
</script>

<style scoped src="./monitor-shared.css"></style>

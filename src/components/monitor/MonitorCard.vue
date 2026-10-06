<template>
    <n-card :title="monitor.name" size="small">
        <template #header-extra
            ><n-switch
                :value="monitor.enabled"
                :disabled="busy"
                @update:value="emit('toggle', $event)"
                ><template #checked>已启用</template
                ><template #unchecked>已停用</template></n-switch
            ></template
        >
        <p>
            {{ sourceLabel(monitor.source) }} · {{ monitor.quality }} · 每
            {{ monitor.intervalMinutes }} 分钟
        </p>
        <p class="muted">
            上次：{{ time(monitor.lastCheck) }} · 下次：{{
                monitor.enabled ? time(monitor.nextCheck, '尽快检查') : '已停用'
            }}
        </p>
        <p>{{ monitor.lastResult }}</p>
        <div
            v-if="monitor.initialProgress.initialized"
            class="initial-progress"
        >
            <strong>首次补齐进度</strong>
            <n-progress
                type="line"
                :percentage="monitor.initialProgress.percent"
                :status="
                    monitor.initialProgress.percent === 100
                        ? 'success'
                        : 'default'
                "
            />
            <p class="muted">
                首次快照 {{ monitor.initialProgress.total }} 首 · 已处理
                {{ monitor.initialProgress.completed }} · 已移出歌单
                {{ monitor.initialProgress.removed }} · 处理中
                {{ monitor.initialProgress.active }} · 待确认
                {{ monitor.initialProgress.confirmation }} · 失败
                {{ monitor.initialProgress.failed }}
            </p>
            <p class="muted">
                已处理包括关联、下载成功和忽略；已移出项不再需要补齐。新加入歌曲单独计入逐曲状态。
            </p>
        </div>
        <p v-else class="muted">首次补齐进度：等待成功读取歌单建立快照。</p>
        <p v-if="monitor.latestRound" class="muted">
            最近一轮 · {{ roundLabel(monitor.latestRound.status) }} · 新增
            {{ monitor.latestRound.added }} · 关联
            {{ monitor.latestRound.linked }} · 入队
            {{ monitor.latestRound.enqueued }} · 派发失败
            {{ monitor.latestRound.failed }}
        </p>
        <n-space
            ><n-tag
                v-for="(count, state) in monitor.counts"
                :key="state"
                :type="stateType(String(state))"
                >{{ stateLabel(String(state)) }} {{ count }}</n-tag
            ></n-space
        >
        <n-space class="actions">
            <n-button :disabled="busy" @click="emit('check')"
                >立即检查</n-button
            >
            <n-button :disabled="busy" @click="emit('edit')">编辑</n-button>
            <n-button
                :type="selected ? 'primary' : 'default'"
                :disabled="busy"
                @click="emit('select')"
                >查看歌曲</n-button
            >
            <n-popconfirm @positive-click="emit('remove')">
                <template #trigger
                    ><n-button type="error" :disabled="busy"
                        >删除监控</n-button
                    ></template
                >
                删除“{{
                    monitor.name
                }}”后停止该监控的后续检查与补齐；已有文件、共享处理台账和下载任务保留。
            </n-popconfirm>
            <n-button :disabled="busy" @click="emit('history')"
                >检查历史</n-button
            >
        </n-space>
    </n-card>
</template>

<script setup lang="ts">
import {
    NButton,
    NCard,
    NPopconfirm,
    NProgress,
    NSpace,
    NSwitch,
    NTag,
} from 'naive-ui'
import type { Monitor } from '../../api/monitorApi'
import {
    time,
    sourceLabel,
    stateLabel,
    stateType,
    roundLabel,
} from './presentation'
defineProps<{ monitor: Monitor; selected: boolean; busy: boolean }>()
const emit = defineEmits<{
    toggle: [enabled: boolean]
    check: []
    edit: []
    select: []
    remove: []
    history: []
}>()
</script>

<style scoped src="./monitor-shared.css"></style>
<style scoped>
.initial-progress {
    margin-top: 12px;
    padding: 12px;
    border: 1px solid var(--border-color);
    border-radius: 8px;
}
</style>

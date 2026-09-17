<template>
    <MobileTaskList v-if="isMobile" :tasks="tasks" :selected-row-keys="selectedRowKeys" :is-android="isAndroid"
        @update:selected-row-keys="(keys) => emit('update:selectedRowKeys', keys)"
        @action="(action, taskId, extra) => emit('action', action, taskId, extra)" />
    <n-data-table v-else class="task-table" :columns="columns" :data="tasks" :row-key="(row: TaskRecord) => row.id"
        :scroll-x="1080" table-layout="fixed"
        :checked-row-keys="(selectedRowKeys as any)"
        @update:checked-row-keys="(keys: any[]) => $emit('update:selectedRowKeys', keys as string[])" />
</template>

<script setup lang="ts">
import { h, ref, onMounted } from 'vue'
import { NDataTable, NTag, NProgress, NSpace, NEllipsis } from 'naive-ui'
import type { DataTableColumn } from 'naive-ui'
import type { TaskRecord } from '../../types'
import { formatSpeed } from '../../utils/format'
import { renderActions } from './TaskRowActions'
import MobileTaskList from './MobileTaskList.vue'
import { useNarrowLayout } from '../../composables/useNarrowLayout'
// 导入 OS 插件，用于获取平台信息
import { platform } from '@tauri-apps/plugin-os'

// 使用 ref 存储 Android 状态，替代原先的同步 UA 判断
// 使用 Tauri OS 插件准确识别平台，在 onMounted 中异步获取平台并更新
const isAndroid = ref(false)

// 响应式检测移动端：与导航共用断点，监听由 composable 随组件释放
const isMobile = useNarrowLayout()

onMounted(async () => {
    // 异步获取当前平台，设置 isAndroid
    try {
        const currentPlatform = await platform()
        isAndroid.value = currentPlatform === 'android'
    } catch (error) {
        console.warn('获取平台信息失败，默认按非 Android 处理', error)
        isAndroid.value = false
    }
})

const props = defineProps<{
    tasks: TaskRecord[]
    selectedRowKeys: string[]
}>()

const emit = defineEmits<{
    (e: 'update:selectedRowKeys', keys: string[]): void
    // 增加第三个参数 extra，用于传递删除文件标志等
    (e: 'action', action: string, taskId: string, extra?: Record<string, any>): void
}>()

/**
 * 渲染进度列
 */
function renderProgress(row: TaskRecord) {
    const percent = row.fileSize > 0 ? Math.round((row.downloaded / row.fileSize) * 100) : 0

    // 已完成或处理中状态：显示100%
    if (row.status === 'completed' || row.status === 'processing') {
        return '100%'
    }

    // 错误状态显示错误信息
    if (row.status === 'error') {
        return row.errorMsg || ''
    }

    // 等待中
    if (row.status === 'waiting') {
        return '-'
    }

    // 下载中或暂停：显示进度条和速度
    const children = [
        h(NProgress, {
            percentage: percent,
            indicatorTextPlacement: 'inside',
            height: 20,
        })
    ]

    if (row.speed && row.speed > 0) {
        children.push(
            h('div', { class: 'task-speed' },
                formatSpeed(row.speed)
            )
        )
    }

    return h('div', null, children)
}

const columns: DataTableColumn<TaskRecord>[] = [
    {
        type: 'selection',
        disabled: (row: TaskRecord) => row.status === 'downloading',
    },
    {
        title: '歌曲信息',
        key: 'song',
        minWidth: 200,  // 确保最小列宽，防止被压缩
        render(row: TaskRecord) {
            return h('div', { class: 'song-info' }, [
                h('span', { class: 'song-title' }, row.songTitle || '未知歌曲'),
                h('span', { class: 'song-separator' }, ' - '),
                h('span', { class: 'song-artist' }, row.artist || '未知歌手'),
            ])
        },
    },
    {
        title: '音质',
        key: 'quality',
        width: 100,
        render(row: TaskRecord) {
            return row.quality
        },
    },
    {
        title: '状态',
        key: 'status',
        width: 100,
        render(row: TaskRecord) {
            const statusMap: Record<string, { type: string; label: string }> = {
                waiting: { type: 'info', label: '等待中' },
                downloading: { type: 'info', label: '下载中' },
                paused: { type: 'warning', label: '暂停' },
                completed: { type: 'success', label: '已完成' },
                error: { type: 'error', label: '错误' },
                processing: { type: 'info', label: '处理中' },
            }
            const s = statusMap[row.status] || { type: 'default', label: row.status }
            return h(NTag, { type: s.type as any, size: 'small' }, () => s.label)
        },
    },
    {
        title: '进度',
        key: 'progress',
        width: 200,
        render(row: TaskRecord) {
            return renderProgress(row)
        },
    },
    // 仅 Android 显示文件路径列
    ...(isAndroid ? [{
        title: '文件路径',
        key: 'filePath',
        minWidth: 200,
        render(row: TaskRecord) {
            const fullPath = row.filePath || '-'
            return h(NEllipsis, {
                style: {
                    fontSize: '12px',
                    maxWidth: '300px',
                },
                expandTrigger: 'click',
                lineClamp: 1,
                tooltip: false, // 禁用 tooltip，改用点击展开
            }, () => fullPath)
        },
    }] : []),
    {
        title: '操作',
        key: 'actions',
        width: 200,
        render(row: TaskRecord) {
            return h(NSpace, { justify: 'center' }, () =>
                renderActions(row, {
                    // 显式传递第三个参数，确保 extra 不被丢弃
                    emit: (action: string, taskId: string, extra?: Record<string, any>) => {
                        emit('action', action, taskId, extra)
                    },
                    isAndroid: isAndroid.value,
                })
            )
        },
    },
]
</script>

<style scoped>
.task-table {
    min-width: 0;
    flex-shrink: 0;
}

/* 固定列布局配合表格内部横向滚动，长内容在单元格内换行或省略 */
.task-table :deep(.n-data-table-td) {
    overflow-wrap: anywhere;
}

.task-table :deep(.task-speed) {
    font-size: 12px;
    color: var(--color-text-secondary);
    margin-top: 4px;
}

/* 列内容由表格的 render 回调创建，使用 deep 将样式限定在当前表格内 */
.task-table :deep(.song-info) {
    display: flex;
    flex-direction: row;
    align-items: baseline;
    flex-wrap: nowrap;
    /* 改为 nowrap，禁止换行 */
    min-width: 0;
    /* 允许自身收缩 */
}

.task-table :deep(.song-title) {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    /* 允许收缩 */
    min-width: 0;
    /* 允许收缩 */
}

.task-table :deep(.song-separator) {
    margin: 0 4px;
    color: var(--color-text-secondary);
    font-size: 12px;
    flex-shrink: 0;
    /* 分隔符不收缩 */
}

.task-table :deep(.song-artist) {
    font-size: 12px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    /* 允许收缩 */
    min-width: 0;
    /* 允许收缩 */
}
</style>

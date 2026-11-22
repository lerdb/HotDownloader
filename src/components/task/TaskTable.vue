<template>
    <MobileTaskList
        v-if="isMobile"
        :tasks="tasks"
        :selected-row-keys="selectedRowKeys"
        :is-android="isAndroid"
        @update:selected-row-keys="
            (keys) => emit('update:selectedRowKeys', keys)
        "
        @action="
            (action, taskId, extra) => emit('action', action, taskId, extra)
        "
    />
    <n-data-table
        v-else
        class="task-table"
        :columns="columns"
        :data="tasks"
        :row-key="(row: TaskRecord) => row.id"
        :scroll-x="1080"
        table-layout="fixed"
        :checked-row-keys="selectedRowKeys"
        @update:checked-row-keys="handleCheckedRowKeys"
    />
</template>

<script setup lang="ts">
import { h, ref, onMounted, computed } from 'vue'
import { NDataTable, NTag, NProgress, NSpace, NEllipsis } from 'naive-ui'
import type { DataTableColumn, DataTableRowKey, TagProps } from 'naive-ui'
import type { TaskRecord } from '../../types'
import { formatSpeed } from '../../utils/format'
import { renderActions } from './TaskRowActions'
import type { TaskAction, TaskActionExtra } from './TaskRowActions'
import MobileTaskList from './MobileTaskList.vue'
import AppIcon from '../common/AppIcon.vue'
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { getRuntimePlatform } from '../../api/runtimeApi'

const isAndroid = ref(false)
const isMobile = useNarrowLayout()

onMounted(async () => {
    try {
        const currentPlatform = await getRuntimePlatform()
        isAndroid.value =
            currentPlatform === 'android' || currentPlatform === 'web'
    } catch (error) {
        console.warn('获取平台信息失败，默认按非 Android 处理', error)
        isAndroid.value = false
    }
})

defineProps<{
    tasks: TaskRecord[]
    selectedRowKeys: string[]
}>()

const emit = defineEmits<{
    (e: 'update:selectedRowKeys', keys: string[]): void
    (
        e: 'action',
        action: TaskAction,
        taskId: string,
        extra?: TaskActionExtra,
    ): void
}>()

function handleCheckedRowKeys(keys: DataTableRowKey[]) {
    emit('update:selectedRowKeys', keys.map(String))
}

/**
 * 渲染进度列
 */
function renderProgress(row: TaskRecord) {
    const percent =
        row.fileSize > 0 ? Math.round((row.downloaded / row.fileSize) * 100) : 0

    if (row.status === 'completed' || row.status === 'processing') {
        return h('span', { class: 'progress-done' }, '100%')
    }

    if (row.status === 'interrupted') {
        return h(
            'span',
            { class: 'progress-note is-warn' },
            '上次运行中断，等待恢复',
        )
    }

    if (row.status === 'error') {
        return h('span', { class: 'progress-note is-error' }, row.errorMsg || '')
    }

    if (row.status === 'waiting') {
        return h('span', { class: 'progress-note' }, '-')
    }

    const children = [
        h(NProgress, {
            percentage: percent,
            indicatorTextPlacement: 'inside',
            height: 18,
            borderRadius: 999,
        }),
    ]

    if (row.speed && row.speed > 0) {
        children.push(h('div', { class: 'task-speed' }, formatSpeed(row.speed)))
    }

    return h('div', null, children)
}

const columns = computed<DataTableColumn<TaskRecord>[]>(() => [
    {
        type: 'selection',
        disabled: (row: TaskRecord) => row.status === 'downloading',
    },
    {
        title: '歌曲信息',
        key: 'song',
        minWidth: 230,
        render(row: TaskRecord) {
            return h('div', { class: 'song-cell' }, [
                row.coverUrl
                    ? h('img', {
                          src: row.coverUrl,
                          alt: row.songTitle,
                          class: 'song-thumb',
                          loading: 'lazy',
                      })
                    : h('div', { class: 'song-thumb song-thumb-fallback' }, [
                          h(AppIcon, { name: 'music', size: 16 }),
                      ]),
                h('div', { class: 'song-info' }, [
                    h(
                        'div',
                        { class: 'song-title', title: row.songTitle },
                        row.songTitle || '未知歌曲',
                    ),
                    h(
                        'div',
                        { class: 'song-artist', title: row.artist },
                        row.artist || '未知歌手',
                    ),
                ]),
            ])
        },
    },
    {
        title: '音质',
        key: 'quality',
        width: 108,
        render(row: TaskRecord) {
            return h('span', { class: 'quality-badge' }, row.quality)
        },
    },
    {
        title: '状态',
        key: 'status',
        width: 100,
        render(row: TaskRecord) {
            const statusMap: Record<
                string,
                { type: TagProps['type']; label: string }
            > = {
                waiting: { type: 'info', label: '等待中' },
                downloading: { type: 'info', label: '下载中' },
                paused: { type: 'warning', label: '暂停' },
                completed: { type: 'success', label: '已完成' },
                error: { type: 'error', label: '错误' },
                processing: { type: 'info', label: '处理中' },
                interrupted: { type: 'warning', label: '已中断' },
            }
            const s = statusMap[row.status] || {
                type: 'default' as const,
                label: row.status,
            }
            return h(
                NTag,
                { type: s.type, size: 'small', round: true, bordered: false },
                () => s.label,
            )
        },
    },
    {
        title: '进度',
        key: 'progress',
        width: 210,
        render(row: TaskRecord) {
            return renderProgress(row)
        },
    },
    ...(isAndroid.value
        ? [
              {
                  title: '文件路径',
                  key: 'filePath',
                  minWidth: 200,
                  render(row: TaskRecord) {
                      const fullPath = row.filePath || '-'
                      return h(
                          NEllipsis,
                          {
                              style: {
                                  fontSize: '12px',
                                  maxWidth: '300px',
                              },
                              expandTrigger: 'click',
                              lineClamp: 1,
                              tooltip: false,
                          },
                          () => fullPath,
                      )
                  },
              },
          ]
        : []),
    {
        title: '操作',
        key: 'actions',
        width: 200,
        render(row: TaskRecord) {
            return h(NSpace, { justify: 'center' }, () =>
                renderActions(row, {
                    emit: (
                        action: TaskAction,
                        taskId: string,
                        extra?: TaskActionExtra,
                    ) => {
                        emit('action', action, taskId, extra)
                    },
                    isAndroid: isAndroid.value,
                }),
            )
        },
    },
])
</script>

<style scoped>
.task-table {
    min-width: 0;
    flex-shrink: 0;
    border-radius: 14px;
    overflow: hidden;
    border: 1px solid var(--border-color);
    box-shadow: var(--shadow-sm);
}

.task-table :deep(.n-data-table-td) {
    overflow-wrap: anywhere;
}

.task-table :deep(.song-cell) {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
}

.task-table :deep(.song-thumb) {
    width: 38px;
    height: 38px;
    border-radius: 8px;
    object-fit: cover;
    flex-shrink: 0;
}

.task-table :deep(.song-thumb-fallback) {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.task-table :deep(.song-info) {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
}

.task-table :deep(.song-title) {
    font-weight: 600;
    font-size: 13.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.task-table :deep(.song-artist) {
    font-size: 12px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 2px;
}

.task-table :deep(.quality-badge) {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
}

.task-table :deep(.task-speed) {
    font-size: 12px;
    color: var(--color-text-secondary);
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
}

.task-table :deep(.progress-done) {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-primary);
    font-variant-numeric: tabular-nums;
}

.task-table :deep(.progress-note) {
    font-size: 12px;
    color: var(--color-text-secondary);
}

.task-table :deep(.progress-note.is-error) {
    color: #ef4444;
}

.task-table :deep(.progress-note.is-warn) {
    color: #f59e0b;
}
</style>

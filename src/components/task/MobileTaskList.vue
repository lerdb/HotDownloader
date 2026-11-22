<template>
    <!-- 该组件使用 render 函数渲染，故 template 为空 -->
</template>

<script lang="ts">
import { defineComponent, h, type PropType } from 'vue'
import {
    NCheckbox,
    NTag,
    NProgress,
    NSpace,
    NEllipsis,
    type TagProps,
} from 'naive-ui'
import type { TaskRecord } from '../../types'
import { renderActions } from './TaskRowActions'
import type { TaskAction, TaskActionExtra } from './TaskRowActions'
import AppIcon from '../common/AppIcon.vue'

export default defineComponent({
    name: 'MobileTaskList',
    props: {
        tasks: {
            type: Array as PropType<TaskRecord[]>,
            required: true,
        },
        selectedRowKeys: {
            type: Array as PropType<string[]>,
            required: true,
        },
        isAndroid: {
            type: Boolean,
            default: false,
        },
    },
    emits: ['update:selectedRowKeys', 'action'],
    setup(props, { emit }) {
        function formatSpeed(bytesPerSec: number): string {
            if (!bytesPerSec || bytesPerSec <= 0) return ''
            const units = ['B/s', 'KB/s', 'MB/s', 'GB/s']
            let unitIndex = 0
            let value = bytesPerSec
            while (value >= 1024 && unitIndex < units.length - 1) {
                value /= 1024
                unitIndex++
            }
            return `${value.toFixed(1)} ${units[unitIndex]}`
        }

        function progressPercent(task: TaskRecord): number {
            if (task.status === 'completed' || task.status === 'processing')
                return 100
            if (task.fileSize > 0) {
                return Math.min(
                    100,
                    Math.round((task.downloaded / task.fileSize) * 100),
                )
            }
            return 0
        }

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

        function actionNodes(task: TaskRecord) {
            return renderActions(task, {
                emit: (
                    action: TaskAction,
                    taskId: string,
                    extra?: TaskActionExtra,
                ) => {
                    emit('action', action, taskId, extra)
                },
                isAndroid: props.isAndroid,
            })
        }

        function toggleSelection(taskId: string, checked: boolean) {
            const newKeys = new Set(props.selectedRowKeys)
            if (checked) {
                newKeys.add(taskId)
            } else {
                newKeys.delete(taskId)
            }
            emit('update:selectedRowKeys', Array.from(newKeys))
        }

        function renderTaskCard(task: TaskRecord) {
            const status = statusMap[task.status] || {
                type: 'default' as const,
                label: task.status,
            }
            const checked = props.selectedRowKeys.includes(task.id)
            const disabled = task.status === 'downloading'

            const cardChildren = [
                h('div', { class: 'task-card-header' }, [
                    h(NCheckbox, {
                        checked,
                        disabled,
                        'onUpdate:checked': (val: boolean) =>
                            toggleSelection(task.id, val),
                    }),
                    task.coverUrl
                        ? h('img', {
                              src: task.coverUrl,
                              alt: task.songTitle,
                              class: 'task-card-cover',
                              loading: 'lazy',
                          })
                        : h(
                              'div',
                              {
                                  class: 'task-card-cover task-card-cover-fallback',
                              },
                              [h(AppIcon, { name: 'music', size: 18 })],
                          ),
                    h('div', { class: 'task-card-song' }, [
                        h(
                            'div',
                            { class: 'task-card-title' },
                            task.songTitle || '未知歌曲',
                        ),
                        h(
                            'div',
                            { class: 'task-card-artist' },
                            task.artist || '未知歌手',
                        ),
                    ]),
                ]),

                h('div', { class: 'task-card-meta' }, [
                    h(
                        NTag,
                        {
                            type: status.type,
                            size: 'small',
                            round: true,
                            bordered: false,
                        },
                        () => status.label,
                    ),
                    h('span', { class: 'task-card-quality' }, task.quality),
                    task.status === 'downloading' &&
                    task.speed &&
                    task.speed > 0
                        ? h(
                              'span',
                              { class: 'task-card-speed' },
                              formatSpeed(task.speed),
                          )
                        : null,
                ]),

                task.status === 'error' || task.status === 'interrupted'
                    ? h(
                          'div',
                          {
                              class: [
                                  'task-card-status-message',
                                  task.status === 'error'
                                      ? 'is-error'
                                      : 'is-interrupted',
                              ],
                          },
                          task.status === 'interrupted'
                              ? '上次运行中断，等待恢复'
                              : task.errorMsg || '下载失败',
                      )
                    : h('div', { class: 'task-card-progress' }, [
                          h(NProgress, {
                              percentage: progressPercent(task),
                              height: 10,
                              borderRadius: 999,
                          }),
                      ]),
            ]

            if (props.isAndroid) {
                cardChildren.push(
                    h('div', { class: 'task-card-filepath' }, [
                        h(
                            NEllipsis,
                            {
                                style: {
                                    fontSize: '12px',
                                    maxWidth: '100%',
                                },
                                expandTrigger: 'click',
                                lineClamp: 1,
                                tooltip: false,
                            },
                            () => task.filePath || '-',
                        ),
                    ]),
                )
            }

            cardChildren.push(
                h('div', { class: 'task-card-actions' }, [
                    h(NSpace, { justify: 'end', wrap: true }, () =>
                        actionNodes(task),
                    ),
                ]),
            )

            return h(
                'div',
                {
                    class: ['task-card', { 'is-selected': checked }],
                    key: task.id,
                },
                cardChildren,
            )
        }

        return () => {
            if (props.tasks.length === 0) {
                return h('div', { class: 'task-card-empty' }, '暂无任务')
            }
            return h(
                'div',
                { class: 'task-card-list' },
                props.tasks.map((task) => renderTaskCard(task)),
            )
        }
    },
})
</script>

<style scoped>
.task-card-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
}

.task-card {
    min-width: 0;
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 16px;
    padding: 14px;
    transition:
        border-color 0.2s,
        background-color 0.2s;
}

.task-card.is-selected {
    border-color: var(--color-primary);
    background: var(--color-primary-soft);
}

.task-card-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
}

.task-card-cover {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    object-fit: cover;
    flex-shrink: 0;
}

.task-card-cover-fallback {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.task-card-song {
    flex: 1;
    min-width: 0;
}

.task-card-title {
    font-weight: 600;
    font-size: 15px;
    line-height: 1.4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-text);
}

.task-card-artist {
    font-size: 13px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 2px;
}

.task-card-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 10px;
}

.task-card-quality {
    max-width: 100%;
    overflow-wrap: anywhere;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-secondary);
    background: var(--bg-surface-subtle);
    padding: 2px 9px;
    border-radius: 999px;
}

.task-card-speed {
    font-size: 12px;
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
}

.task-card-progress {
    margin-bottom: 8px;
}

.task-card-filepath {
    min-width: 0;
    overflow-wrap: anywhere;
    margin-bottom: 8px;
    padding: 6px 10px;
    background: var(--bg-surface-subtle);
    border-radius: 8px;
    font-size: 12px;
    color: var(--color-text-secondary);
}

.task-card-status-message {
    margin-bottom: 8px;
    padding: 6px 10px;
    background: var(--bg-surface-subtle);
    border-radius: 8px;
    font-size: 12px;
    word-break: break-word;
    overflow-wrap: anywhere;
    line-height: 1.6;
}

.task-card-status-message.is-error {
    color: var(--n-error-color, #ef4444);
}

.task-card-status-message.is-interrupted {
    color: var(--n-warning-color, #f59e0b);
}

.task-card-actions {
    padding-top: 6px;
}

.task-card-actions :deep(.n-button) {
    min-height: 40px;
    padding-left: 14px;
    padding-right: 14px;
}

.task-card-empty {
    text-align: center;
    padding: 48px 0;
    color: var(--color-text-secondary);
    font-size: 14px;
}
</style>

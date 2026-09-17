import { h, ref, type VNode } from 'vue'
import { NButton, NPopconfirm, NCheckbox, NSpace } from 'naive-ui'
import type { TaskRecord } from '../../types'

export interface TaskActionContext {
    emit: (action: string, taskId: string, extra?: Record<string, any>) => void
    isAndroid?: boolean
}

export function renderActions(
    task: TaskRecord,
    context: TaskActionContext
): VNode[] {
    const { emit, isAndroid = false } = context
    const taskId = task.id
    const nodes: VNode[] = []

    // 处理中：不提供任何操作，避免中断元数据写入或歌词处理
    if (task.status === 'processing') {
        return nodes
    }

    // 等待中：取消（可能文件尚未创建，但用户可选择同时删除潜在的空文件）
    if (task.status === 'waiting') {
        nodes.push(
            createCancelWithDeletePopconfirm(emit, taskId)
        )
    }

    // 下载中：暂停（不弹窗询问删除）
    if (task.status === 'downloading') {
        nodes.push(
            h(
                NButton,
                {
                    size: 'small',
                    type: 'warning',
                    onClick: () => emit('pause', taskId),
                },
                () => '暂停'
            )
        )
    }

    // 暂停：恢复、取消（取消时询问删除文件）
    if (task.status === 'paused') {
        nodes.push(
            h(
                NButton,
                {
                    size: 'small',
                    type: 'primary',
                    onClick: () => emit('resume', taskId),
                },
                () => '恢复'
            )
        )
        nodes.push(
            createCancelWithDeletePopconfirm(emit, taskId)
        )
    }

    // 错误：重试、删除（删除时询问是否删除未完成文件）
    if (task.status === 'error') {
        const isRetriable =
            task.errorMsg !== '重试次数已用尽' &&
            task.errorMsg !== '已无更低音质可降级'

        nodes.push(
            h(
                NButton,
                {
                    size: 'small',
                    type: 'primary',
                    disabled: !isRetriable,
                    onClick: () => {
                        if (isRetriable) emit('retry', taskId)
                    },
                },
                () => '重试'
            )
        )
        nodes.push(
            createRemoveWithDeletePopconfirm(
                emit,
                taskId,
                '确定删除该任务记录吗？',
                '同时删除未下载完成的文件',
                true // 默认勾选，因为错误文件通常无保留价值
            )
        )
    }

    // 已完成：打开文件位置、删除（删除时询问是否删除已下载文件）
    if (task.status === 'completed') {
        if (!isAndroid) {
            nodes.push(
                h(
                    NButton,
                    {
                        size: 'small',
                        onClick: () => emit('open-location', taskId),
                    },
                    () => '打开文件位置'
                )
            )
        }
        nodes.push(
            createRemoveWithDeletePopconfirm(
                emit,
                taskId,
                '确定删除该任务记录吗？',
                '同时删除已下载的文件',
                false // 默认不勾选，保护已下载文件
            )
        )
    }

    return nodes
}

/**
 * 创建带“删除文件”选项的取消确认弹窗（用于 waiting/paused 状态）
 */
function createCancelWithDeletePopconfirm(
    emit: TaskActionContext['emit'],
    taskId: string
) {
    const deleteFile = ref(false)

    return h(
        NPopconfirm,
        {
            style: { maxWidth: 'calc(100vw - 32px)' },
            onPositiveClick: () => {
                emit('cancel', taskId, { deleteFile: deleteFile.value })
            },
        },
        {
            trigger: () =>
                h(NButton, { size: 'small', type: 'warning' }, () => '取消'),
            default: () => {
                return h(NSpace, { vertical: true, size: 'small' }, () => [
                    h('span', {}, '确定取消该任务吗？'),
                    h(
                        NCheckbox,
                        {
                            checked: deleteFile.value,
                            'onUpdate:checked': (val: boolean) => {
                                deleteFile.value = val
                            },
                        },
                        () => '同时删除未下载完成的文件'
                    ),
                ])
            },
        }
    )
}

/**
 * 创建带“删除文件”选项的删除确认弹窗（用于 error/completed 状态）
 * @param emit 事件发射器
 * @param taskId 任务 ID
 * @param confirmText 确认提示语
 * @param checkboxLabel 复选框标签
 * @param defaultChecked 复选框默认是否勾选
 */
function createRemoveWithDeletePopconfirm(
    emit: TaskActionContext['emit'],
    taskId: string,
    confirmText: string,
    checkboxLabel: string,
    defaultChecked: boolean
) {
    const deleteFile = ref(defaultChecked)

    return h(
        NPopconfirm,
        {
            style: { maxWidth: 'calc(100vw - 32px)' },
            onPositiveClick: () => {
                emit('remove', taskId, { deleteFile: deleteFile.value })
            },
        },
        {
            trigger: () =>
                h(NButton, { size: 'small', type: 'error' }, () => '删除'),
            default: () => {
                return h(NSpace, { vertical: true, size: 'small' }, () => [
                    h('span', {}, confirmText),
                    h(
                        NCheckbox,
                        {
                            checked: deleteFile.value,
                            'onUpdate:checked': (val: boolean) => {
                                deleteFile.value = val
                            },
                        },
                        () => checkboxLabel
                    ),
                ])
            },
        }
    )
}

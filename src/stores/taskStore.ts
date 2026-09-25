import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
    SongInfo,
    TaskRecord,
    DownloadMetadataErrorPayload,
    CreateTaskRequest,
    CreateTaskResult,
    DuplicateAction,
} from '../types'

interface BatchResult {
    succeeded: number
    failed: number
    errors: string[]
}

/** Rust 持有任务和状态流转；此 store 仅保存页面展示用的投影。 */
export const useTaskStore = defineStore('tasks', () => {
    const tasks = ref<TaskRecord[]>([])
    let loading = false
    let pendingEvents: Array<() => void> = []

    function projectEvent(apply: () => void) {
        // 加载快照期间收到的事件必须排队，快照落地后再重放，避免覆盖较新的状态。
        if (loading) {
            pendingEvents.push(apply)
        } else {
            apply()
        }
    }

    function upsert(task: TaskRecord) {
        const index = tasks.value.findIndex(t => t.id === task.id)
        if (index < 0) {
            tasks.value.push(task)
        } else {
            tasks.value[index] = task
        }
    }

    async function loadTasks() {
        loading = true
        try {
            tasks.value = await invoke<TaskRecord[]>('load_tasks')
        } finally {
            loading = false
            // 加载失败时也要重放事件，避免页面永远停留在旧状态。
            const queued = pendingEvents
            pendingEvents = []
            queued.forEach(apply => apply())
        }
    }

    function createTask(song: SongInfo, desiredQuality: string, duplicateAction?: DuplicateAction) {
        const request: CreateTaskRequest = { song, desiredQuality, duplicateAction }
        // 不在前端预写任务；命令返回及 task-updated 事件都以 Rust 的记录为准。
        return invoke<CreateTaskResult>('create_download_task', {
            request,
        })
    }

    async function cancelTask(taskId: string, deleteFile = false) {
        await invoke('cancel_task', { taskId, deleteFile })
    }

    async function removeTasks(taskIds: string[], deleteFile = false): Promise<BatchResult> {
        if (taskIds.length === 0) {
            return { succeeded: 0, failed: 0, errors: [] }
        }
        return invoke<BatchResult>('remove_tasks', { taskIds, deleteFile })
    }

    async function removeTask(taskId: string, deleteFile = false) {
        return removeTasks([taskId], deleteFile)
    }

    async function pauseTask(taskId: string) {
        await invoke('pause_task', { taskId })
    }

    async function resumeTask(taskId: string) {
        await invoke('resume_task', { taskId })
    }

    function retryTask(taskId: string): Promise<boolean> {
        // 后端统一处理重试次数、断点续传及音质降级；false 表示当前无法继续重试。
        return invoke<boolean>('retry_task', { taskId })
    }

    /** 逐个发起重试命令，实际下载并发仍由 Rust 调度器控制。 */
    async function retryTasks(taskIds: string[]): Promise<{ total: number; succeeded: number; failed: number }> {
        let succeeded = 0
        let failed = 0
        for (const id of taskIds) {
            try {
                if (await retryTask(id)) {
                    succeeded++
                } else {
                    failed++
                }
            } catch (error) {
                console.error('重试任务失败:', id, error)
                failed++
            }
        }
        return { total: taskIds.length, succeeded, failed }
    }

    async function setupListeners(): Promise<() => void> {
        const registrations: Array<Promise<UnlistenFn>> = []
        registrations.push(listen<TaskRecord>('task-updated', event => {
            projectEvent(() => {
                const previous = tasks.value.find(t => t.id === event.payload.id)
                upsert(event.payload)

                // 只在状态变化时提醒，下载进度事件不会反复弹通知。
                if (previous && previous.status !== event.payload.status) {
                    if (event.payload.status === 'completed') {
                        window.$notify?.success({
                            title: '下载完成',
                            description: `歌曲“${event.payload.songTitle}”已下载完成`,
                            duration: 3000,
                        })
                    } else if (event.payload.status === 'error') {
                        window.$notify?.error({
                            title: '下载失败',
                            description: `歌曲“${event.payload.songTitle}”错误：${(event.payload.errorMsg || '').slice(0, 100)}`,
                            duration: 3000,
                        })
                    }
                }
            })
        }))
        registrations.push(listen<string>('task-removed', event => {
            projectEvent(() => {
                tasks.value = tasks.value.filter(t => t.id !== event.payload)
            })
        }))
        // 元数据写入失败不等于文件下载失败，保留独立的提示事件。
        registrations.push(listen<DownloadMetadataErrorPayload>('download-metadata-error', event => {
            const task = tasks.value.find(t => t.id === event.payload.task_id)
            window.$notify?.warning({
                title: '元数据写入失败',
                description: `歌曲“${task?.songTitle ?? event.payload.task_id}”元数据写入失败：${event.payload.error_msg}`,
                duration: 3000,
            })
        }))
        registrations.push(listen<string>('login-refresh-failed', event => {
            window.$notify?.error({
                title: '登录刷新失败，请重新登录',
                description: event.payload,
                duration: 5000,
            })
        }))
        const unlisteners = await Promise.all(registrations)
        // 页面退出时统一取消监听，避免重复订阅和重复通知。
        return () => {
            unlisteners.forEach(unlisten => unlisten())
        }
    }

    return {
        tasks,
        loadTasks,
        createTask,
        cancelTask,
        removeTasks,
        removeTask,
        pauseTask,
        resumeTask,
        retryTask,
        retryTasks,
        setupListeners,
    }
})

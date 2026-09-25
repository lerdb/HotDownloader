import { defineStore } from 'pinia'
import { ref } from 'vue'
import { taskTransport } from '../api/taskTransport'
import type { BatchTaskResult } from '../api/taskTransport'
import type {
    SongInfo,
    TaskRecord,
    CreateTaskRequest,
    DuplicateAction,
} from '../types'

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
            tasks.value = await taskTransport.list()
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
        return taskTransport.create(request)
    }

    async function cancelTask(taskId: string, deleteFile = false) {
        await taskTransport.cancel(taskId, deleteFile)
    }

    async function removeTasks(taskIds: string[], deleteFile = false): Promise<BatchTaskResult> {
        if (taskIds.length === 0) {
            return { succeeded: 0, failed: 0, errors: [] }
        }
        return taskTransport.remove(taskIds, deleteFile)
    }

    async function removeTask(taskId: string, deleteFile = false) {
        return removeTasks([taskId], deleteFile)
    }

    async function pauseTask(taskId: string) {
        await taskTransport.pause(taskId)
    }

    async function resumeTask(taskId: string) {
        await taskTransport.resume(taskId)
    }

    function retryTask(taskId: string): Promise<boolean> {
        // 后端统一处理重试次数、断点续传及音质降级；false 表示当前无法继续重试。
        return taskTransport.retry(taskId)
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
        // 从开始订阅到快照加载完成都缓存事件，避免初始化期间的事件被快照覆盖。
        loading = true
        return taskTransport.subscribe({
            snapshot(snapshot) {
                // SSE 重连会重新发完整快照；由后端记录直接替换只读投影。
                projectEvent(() => { tasks.value = snapshot })
            },
            updated(task) {
                projectEvent(() => {
                    const previous = tasks.value.find(t => t.id === task.id)
                    upsert(task)

                    // 只在状态变化时提醒，下载进度事件不会反复弹通知。
                    if (previous && previous.status !== task.status) {
                        if (task.status === 'completed') {
                            window.$notify?.success({
                                title: '下载完成',
                                description: `歌曲“${task.songTitle}”已下载完成`,
                                duration: 3000,
                            })
                        } else if (task.status === 'error') {
                            window.$notify?.error({
                                title: '下载失败',
                                description: `歌曲“${task.songTitle}”错误：${(task.errorMsg || '').slice(0, 100)}`,
                                duration: 3000,
                            })
                        }
                    }
                })
            },
            removed(taskId) {
                projectEvent(() => {
                    tasks.value = tasks.value.filter(t => t.id !== taskId)
                })
            },
            // 元数据写入失败不等于文件下载失败，保留独立的提示事件。
            metadataError(error) {
                const task = tasks.value.find(t => t.id === error.task_id)
                window.$notify?.warning({
                    title: '元数据写入失败',
                    description: `歌曲“${task?.songTitle ?? error.task_id}”元数据写入失败：${error.error_msg}`,
                    duration: 3000,
                })
            },
            loginRefreshFailed(message) {
                window.$notify?.error({
                    title: '登录刷新失败，请重新登录',
                    description: message,
                    duration: 5000,
                })
            },
        })
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

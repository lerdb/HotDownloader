import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
    TaskRecord,
    DownloadProgressPayload,
    DownloadCompletedPayload,
    DownloadErrorPayload,
    DownloadLinkExpiredPayload,
    DownloadFileCompletePayload,
    DownloadMetadataErrorPayload
} from '../types'
import { QUALITY_DOWNGRADE_ORDER } from '../types'
import { useSettingsStore } from './settingsStore'

export const useTaskStore = defineStore('tasks', () => {
    const tasks = ref<TaskRecord[]>([])

    // 简易全局通知辅助函数
    const notify = () => (window as any).$notify

    // ---- 持久化加载 ----
    async function loadTasks() {
        try {
            const json = await invoke<string>('load_tasks')
            if (json) {
                const parsed: TaskRecord[] = JSON.parse(json)
                for (const task of parsed) {
                    if (
                        task.status === 'waiting' ||
                        task.status === 'downloading' ||
                        task.status === 'paused'
                    ) {
                        task.status = 'error'
                        task.errorMsg = '应用关闭导致中断'
                        task.downloaded = 0
                    } else if (task.status === 'error') {
                        task.downloaded = 0
                    }
                }
                tasks.value = parsed
                await saveTasks()
            }
        } catch (e: any) {
            console.error('加载任务失败:', e)
            notify()?.error({ title: '加载任务失败', description: e?.message || String(e), duration: 3000 })
        }
    }

    /**
     * 持久化写的合并（coalescing）控制。
     *
     * 背景：data.json 由 tauri-plugin-store 管理，每次保存都会把整个 store
     * 序列化（且是 pretty print）后整体覆盖写盘。任务数量很多时，这一步是
     * 毫秒级到百毫秒级的重操作。若每次状态变化都直接写盘（例如下载进度事件
     * 每 500ms 一次、批量添加/删除任务时每个任务一次），会瞬间产生大量整表
     * 写盘，把磁盘与 IPC 打满，界面直接卡死。
     *
     * 因此这里保证：任意时刻最多只有一次写入在途；写入期间产生的新变更合并到
     * 下一轮（最多再写一次），所有调用方在“自己那次变更已经落盘”后统一 resolve。
     */
    let saveInFlight = false
    let saveDirty = false
    let saveWaiters: Array<() => void> = []

    async function persistTasks() {
        try {
            await invoke('save_tasks', {
                tasksJson: JSON.stringify(tasks.value),
            })
        } catch (e: any) {
            console.error('保存任务失败:', e)
            notify()?.error({ title: '保存任务失败', description: e?.message || String(e), duration: 3000 })
        }
    }

    async function drainSaveQueue() {
        if (saveInFlight) return
        saveInFlight = true
        try {
            // 循环直到没有新的变更：期间产生的变更会被合并进下一轮写入
            while (saveDirty) {
                saveDirty = false
                await persistTasks()
            }
        } finally {
            saveInFlight = false
            const waiters = saveWaiters
            saveWaiters = []
            waiters.forEach((resolve) => resolve())
        }
    }

    function saveTasks(): Promise<void> {
        saveDirty = true
        const promise = new Promise<void>((resolve) => {
            saveWaiters.push(resolve)
        })
        void drainSaveQueue()
        return promise
    }

    // ---- 任务操作 ----

    /** 后端在执行 enqueue_task 时找不到任务上下文（应用重启后从磁盘恢复的任务） */
    const ERR_TASK_CONTEXT_MISSING = 'TASK_CONTEXT_MISSING'

    function isTaskContextMissing(e: any): boolean {
        const msg = typeof e === 'string' ? e : (e?.message ?? String(e ?? ''))
        return msg.includes(ERR_TASK_CONTEXT_MISSING)
    }

    /**
     * 调用后端注册下载任务：创建（或覆盖）引擎中的任务上下文并加入就绪队列。
     * 下载链接会在下载线程中实时获取，所以这里 url/key 传空字符串即可。
     */
    function registerDownloadTask(task: TaskRecord): Promise<void> {
        return invoke('add_download_task', {
            taskId: task.id,
            platform: task.platform, // 从任务记录中获取平台
            songId: task.songId,
            songMid: task.songMid,
            url: '',
            savePath: task.savePath ?? '',
            quality: task.quality,
            filename: task.filename,   // 传递品质文件名
            key: '',
            fileSize: task.fileSize,
            songTitle: task.songTitle,
            artist: task.artist,
            album: task.album,
            coverUrl: task.coverUrl,
        })
    }

    function addTask(task: TaskRecord) {
        tasks.value.push(task)
        saveTasks()
        registerDownloadTask(task).catch((e: any) => {
            console.error('添加任务失败:', e)
            notify()?.error({ title: '添加任务失败', description: e?.message || String(e), duration: 3000 })
        })
    }

    function cancelTask(taskId: string, deleteFile?: boolean) {
        invoke('cancel_task', { taskId, deleteFile: deleteFile ?? false })
            .catch((e: any) => {
                console.error('取消任务失败:', e)
                notify()?.error({ title: '取消任务失败', description: e?.message || String(e), duration: 3000 })
            })
        removeFromList([taskId])
        saveTasks()
    }

    /** 从内存列表中移除指定任务（使用 Set，避免批量操作退化成 O(n²)） */
    function removeFromList(taskIds: string[]) {
        const idSet = new Set(taskIds)
        tasks.value = tasks.value.filter((t) => !idSet.has(t.id))
    }

    /**
     * 批量移除任务，统一由后端处理文件删除（包括 SAF 模式）。
     * 前端先乐观地把任务从列表移除（界面立刻响应），再调用一次后端批量命令，
     * 最后只做一次整表落盘 —— 避免逐个任务 invoke + 逐个整表写盘。
     */
    async function removeTasks(taskIds: string[], deleteFile: boolean = false) {
        if (taskIds.length === 0) return
        removeFromList(taskIds)
        const saved = saveTasks()
        try {
            await invoke('remove_tasks', { taskIds, deleteFile })
        } catch (e: any) {
            console.error('移除任务失败:', e)
            notify()?.error({ title: '移除任务失败', description: e?.message || String(e), duration: 3000 })
        }
        await saved
    }

    // 移除单个任务
    async function removeTask(taskId: string, deleteFile: boolean = false) {
        await removeTasks([taskId], deleteFile)
    }

    // 现在 enqueueTask 等待后端结果，并处理错误
    async function enqueueTask(taskId: string, offset: number): Promise<boolean> {
        const task = tasks.value.find((t) => t.id === taskId)
        try {
            await invoke('enqueue_task', { taskId, offset })
            return true
        } catch (e: any) {
            // 引擎里没有这个任务的上下文（多是应用重启后从磁盘恢复的任务）。
            // 此时必须用任务记录重新注册一次，否则任务会永远停在“等待中”。
            if (task && isTaskContextMissing(e)) {
                try {
                    console.warn('引擎中缺少任务上下文，重新注册任务:', taskId)
                    await registerDownloadTask(task)
                    return true
                } catch (e2: any) {
                    console.error('重新注册任务失败:', e2)
                    notify()?.error({ title: '重试失败', description: e2?.message || String(e2), duration: 3000 })
                }
            } else {
                console.error('重新入队失败:', e)
                notify()?.error({ title: '重新入队失败', description: e?.message || String(e), duration: 3000 })
            }
            // 入队彻底失败：把任务恢复为 error，避免一直显示“等待中”
            if (task && task.status === 'waiting') {
                task.status = 'error'
                task.errorMsg = '启动下载失败，请稍后重试'
                await saveTasks()
            }
            return false
        }
    }

    function pauseTask(taskId: string) {
        invoke('pause_task', { taskId })
            .catch((e: any) => {
                console.error('暂停任务失败:', e)
                notify()?.error({ title: '暂停任务失败', description: e?.message || String(e), duration: 3000 })
            })
        const task = tasks.value.find((t) => t.id === taskId)
        if (task && task.status === 'downloading') {
            task.status = 'paused'
            saveTasks()
        }
    }

    function resumeTask(taskId: string) {
        invoke('resume_task', { taskId })
            .catch((e: any) => {
                console.error('恢复任务失败:', e)
                notify()?.error({ title: '恢复任务失败', description: e?.message || String(e), duration: 3000 })
            })
        const task = tasks.value.find((t) => t.id === taskId)
        if (task && task.status === 'paused') {
            task.status = 'downloading'
            saveTasks()
        }
    }

    /**
     * 重试 / 降级逻辑，等待 enqueue 结果
     * 返回 true 表示可继续重试（调用方需重新获取链接）
     * 返回 false 表示已永久失败，不可再重试
     */
    async function retryTask(taskId: string): Promise<boolean> {
        const task = tasks.value.find((t) => t.id === taskId)
        if (!task || task.status !== 'error') return false

        task.retryCount += 1

        if (task.retryCount > 3) {
            const settingsStore = useSettingsStore()
            if (settingsStore.settings.autoDowngrade) {
                const currentIdx = QUALITY_DOWNGRADE_ORDER.indexOf(task.quality)
                if (currentIdx >= 0 && currentIdx < QUALITY_DOWNGRADE_ORDER.length - 1) {
                    task.quality = QUALITY_DOWNGRADE_ORDER[currentIdx + 1]
                    task.retryCount = 0
                    task.errorMsg = `自动降级至 ${task.quality}`
                    task.downloaded = 0 // 文件不同，必须重新下载
                } else {
                    task.errorMsg = '已无更低音质可降级'
                    await saveTasks()
                    return false
                }
            } else {
                task.errorMsg = '重试次数已用尽'
                await saveTasks()
                return false
            }
        }

        task.status = 'waiting'
        // 清除旧错误信息
        task.errorMsg = undefined
        await saveTasks()

        const success = await enqueueTask(taskId, task.downloaded)
        if (!success) {
            // enqueueTask 内部已将状态设为 error，但需确保错误信息正确
            const t = tasks.value.find((t) => t.id === taskId)
            if (t && t.status === 'error' && !t.errorMsg) {
                t.errorMsg = '启动下载失败'
            }
            return false
        }
        return true
    }

    /**
     * 批量重试（“全部重试”）。逐个复用 retryTask，保证降级/计数逻辑一致；
     * 实际的并发下载仍由后端调度器按最大并发数排队，前端只负责入队。
     * 返回成功入队 / 失败（例如重试次数用尽）的数量。
     */
    async function retryTasks(taskIds: string[]): Promise<{ total: number; succeeded: number; failed: number }> {
        let succeeded = 0
        let failed = 0
        for (const taskId of taskIds) {
            const ok = await retryTask(taskId)
            if (ok) succeeded++
            else failed++
        }
        return { total: taskIds.length, succeeded, failed }
    }

    function errorTask(taskId: string, errorMsg: string) {
        const task = tasks.value.find((t) => t.id === taskId)
        if (task) {
            task.status = 'error'
            task.errorMsg = errorMsg
            saveTasks()
        }
    }

    // ---- 事件监听 ----
    // 收集所有事件监听器返回的取消函数，并返回一个清理函数用于统一清理，避免内存泄漏和重复监听
    function setupListeners(): () => void {
        const unlisteners: Array<Promise<UnlistenFn>> = []

        unlisteners.push(
            listen<DownloadProgressPayload>('download-progress', (event) => {
                const task = tasks.value.find((t) => t.id === event.payload.task_id)
                if (!task) return
                task.downloaded = event.payload.downloaded
                task.fileSize = event.payload.total
                task.speed = event.payload.speed   // 保存速度
                // 如果任务尚未处于 downloading，则切换为 downloading
                if (task.status !== 'downloading') {
                    task.status = 'downloading'
                    // 状态变化才需要落盘；纯进度（downloaded/speed）是易失数据，
                    // 加载时会重置，因此不写盘，避免每个进度事件都整表写盘。
                    saveTasks()
                }
            })
        )

        // 监听文件下载完成，进入处理中状态
        unlisteners.push(
            listen<DownloadFileCompletePayload>('download-file-complete', (event) => {
                const task = tasks.value.find((t) => t.id === event.payload.task_id)
                if (!task) return
                task.downloaded = task.fileSize
                task.status = 'processing'
                saveTasks()
            })
        )

        // 监听元数据写入失败
        unlisteners.push(
            listen<DownloadMetadataErrorPayload>('download-metadata-error', (event) => {
                const task = tasks.value.find((t) => t.id === event.payload.task_id)
                if (!task) return
                notify()?.warning({
                    title: '元数据写入失败',
                    description: `歌曲“${task.songTitle}”元数据写入失败：${event.payload.error_msg}`,
                    duration: 3000
                })
            })
        )

        // 监听下载完成
        unlisteners.push(
            listen<DownloadCompletedPayload>('download-completed', (event) => {
                const task = tasks.value.find((t) => t.id === event.payload.task_id)
                if (!task) return

                task.status = 'completed'
                // SAF 模式下 final_path 已经是完整 URI，无需额外处理
                task.filePath = event.payload.final_path
                task.downloaded = task.fileSize

                saveTasks()
                // 成功通知
                notify()?.success({
                    title: '下载完成',
                    description: `歌曲“${task.songTitle}”已下载完成`,
                    duration: 3000
                })
            })
        )

        unlisteners.push(
            listen<DownloadErrorPayload>('download-error', (event) => {
                const task = tasks.value.find((t) => t.id === event.payload.task_id)
                if (!task) return
                task.status = 'error'
                task.errorMsg = event.payload.error_msg
                saveTasks()
                // 错误信息可能包含底层网络细节（如长 URL、堆栈等），过长会撑坏通知 UI，故截断显示
                const maxLen = 100
                const displayMsg = event.payload.error_msg.length > maxLen
                    ? event.payload.error_msg.slice(0, maxLen) + '...'
                    : event.payload.error_msg
                // 弹出错误通知
                notify()?.error({
                    title: '下载失败',
                    description: `歌曲“${task.songTitle}”错误：${displayMsg}`,
                    duration: 3000
                })
            })
        )

        unlisteners.push(
            listen<DownloadLinkExpiredPayload>('download-link-expired', (event) => {
                const task = tasks.value.find((t) => t.id === event.payload.task_id)
                if (!task) return
                task.status = 'error'
                task.errorMsg = '链接过期'
                task.downloaded = event.payload.current_offset
                saveTasks()
                // 弹出链接过期通知
                notify()?.warning({
                    title: '链接过期',
                    description: `歌曲“${task.songTitle}”下载链接过期，请稍后重试`,
                    duration: 3000
                })
            })
        )

        // 监听登录刷新失败事件，弹窗提示用户重新登录
        unlisteners.push(
            listen<string>('login-refresh-failed', (event) => {
                notify()?.error({
                    title: '登录刷新失败，请重新登录',
                    description: event.payload,
                    duration: 5000
                })
            })
        )

        // 返回清理函数：异步调用每个 UnlistenFn，触发注销，然后清空数组
        return () => {
            unlisteners.forEach((unlistenPromise) => {
                unlistenPromise.then((unlisten) => unlisten()).catch((e) => {
                    console.error('移除事件监听器失败:', e)
                })
            })
            unlisteners.length = 0
        }
    }

    return {
        tasks,
        loadTasks,
        saveTasks,
        addTask,
        cancelTask,
        removeTask,
        removeTasks,
        enqueueTask,
        pauseTask,
        resumeTask,
        retryTask,
        retryTasks,
        errorTask,
        setupListeners,
    }
})
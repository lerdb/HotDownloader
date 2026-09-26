import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { BatchTaskResult, TaskTransport } from './taskTransport'
import type { SettingsSnapshot } from './settingsApi'
import type {
    CreateTaskResult,
    DownloadMetadataErrorPayload,
    TaskRecord,
} from '../types'

/** 将现有 command 和事件名称集中在一个位置。 */
export const tauriTaskTransport: TaskTransport = {
    list() {
        return invoke<TaskRecord[]>('load_tasks')
    },

    create(request) {
        return invoke<CreateTaskResult>('create_download_task', { request })
    },

    pause(taskId) {
        return invoke<void>('pause_task', { taskId })
    },

    resume(taskId) {
        return invoke<void>('resume_task', { taskId })
    },

    cancel(taskId, deleteFile) {
        return invoke<void>('cancel_task', { taskId, deleteFile })
    },

    remove(taskIds, deleteFile) {
        return invoke<BatchTaskResult>('remove_tasks', { taskIds, deleteFile })
    },

    retry(taskId) {
        return invoke<boolean>('retry_task', { taskId })
    },

    async subscribe(handlers) {
        const unlisteners: UnlistenFn[] = []
        try {
            // 逐个注册，以便后续注册失败时取消已经成功的监听器。
            unlisteners.push(await listen<SettingsSnapshot>('settings-updated', event => {
                handlers.settings(event.payload)
            }))
            unlisteners.push(await listen<TaskRecord>('task-updated', event => {
                handlers.updated(event.payload)
            }))
            unlisteners.push(await listen<string>('task-removed', event => {
                handlers.removed(event.payload)
            }))
            unlisteners.push(await listen<DownloadMetadataErrorPayload>('download-metadata-error', event => {
                handlers.metadataError(event.payload)
            }))
            unlisteners.push(await listen<string>('login-refresh-failed', event => {
                handlers.loginRefreshFailed(event.payload)
            }))
        } catch (error) {
            unlisteners.forEach(unlisten => unlisten())
            throw error
        }

        handlers.connection('connected')

        return () => {
            unlisteners.forEach(unlisten => unlisten())
            handlers.connection('disconnected')
        }
    },
}

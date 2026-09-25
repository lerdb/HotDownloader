import type {
    CreateTaskRequest,
    CreateTaskResult,
    DownloadMetadataErrorPayload,
    TaskRecord,
} from '../types'
import { tauriTaskTransport } from './tauriTaskTransport'
import { webTaskTransport } from './webTaskTransport'
import { isTauri } from '@tauri-apps/api/core'

/** 与 Rust 任务契约对应的批量操作结果。 */
export interface BatchTaskResult {
    succeeded: number
    failed: number
    errors: string[]
}

/** 任务事件携带后端完整记录；前端不得在这里自行推导状态变化。 */
export interface TaskEventHandlers {
    snapshot(tasks: TaskRecord[]): void
    updated(task: TaskRecord): void
    removed(taskId: string): void
    metadataError(error: DownloadMetadataErrorPayload): void
    loginRefreshFailed(message: string): void
}

/** 页面与任务后端的调用边界，与 Tauri command 名称无关。 */
export interface TaskTransport {
    list(): Promise<TaskRecord[]>
    create(request: CreateTaskRequest): Promise<CreateTaskResult>
    pause(taskId: string): Promise<void>
    resume(taskId: string): Promise<void>
    cancel(taskId: string, deleteFile: boolean): Promise<void>
    remove(taskIds: string[], deleteFile: boolean): Promise<BatchTaskResult>
    retry(taskId: string): Promise<boolean>
    subscribe(handlers: TaskEventHandlers): Promise<() => void>
}

// 桌面与移动端走 Tauri IPC，浏览器走 HTTP/SSE；页面和 store 只依赖此接口。
export const taskTransport: TaskTransport = isTauri() ? tauriTaskTransport : webTaskTransport

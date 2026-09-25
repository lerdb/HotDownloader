import type {
    CreateTaskRequest,
    CreateTaskResult,
    DownloadMetadataErrorPayload,
    TaskRecord,
} from '../types'
import { tauriTaskTransport } from './tauriTaskTransport'

/** 与 Rust 任务契约对应的批量操作结果。 */
export interface BatchTaskResult {
    succeeded: number
    failed: number
    errors: string[]
}

/** 任务事件携带后端完整记录；前端不得在这里自行推导状态变化。 */
export interface TaskEventHandlers {
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

// 当前桌面和移动端共用 Tauri 实现；页面和 store 只依赖上面的接口。
export const taskTransport: TaskTransport = tauriTaskTransport

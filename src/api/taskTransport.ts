import type {
    CreateTaskRequest,
    CreateTaskResult,
    DownloadMetadataErrorPayload,
    TaskRecord,
} from '../types'
import { tauriTaskTransport } from './tauriTaskTransport'
import { webTaskTransport } from './webTaskTransport'
import { isTauri } from '@tauri-apps/api/core'
import type { SettingsSnapshot } from './settingsApi'

/** 与 Rust 任务契约对应的批量操作结果。 */
export interface BatchTaskResult {
    succeeded: number
    failed: number
    errors: string[]
}

/** 任务事件携带后端完整记录；前端不得在这里自行推导状态变化。 */
export interface TaskEventHandlers {
    /** Web SSE 的连接状态；原生端建立监听后视为已连接。 */
    connection(status: 'connecting' | 'connected' | 'reconnecting' | 'disconnected'): void
    /** 收到事件或心跳的时间，用于显示当前进度最近一次得到服务响应的时刻。 */
    activity(at: number): void
    settings(snapshot: SettingsSnapshot): void
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

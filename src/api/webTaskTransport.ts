import type { TaskRecord, DownloadMetadataErrorPayload } from '../types'
import type { TaskTransport } from './taskTransport'
import type { SettingsSnapshot } from './settingsApi'
import { webHeaders, webRequest, webSession } from './webClient'

/** 解析一条 SSE 消息。数据始终是服务端序列化的 JSON。 */
function dispatchEvent(frame: string, handlers: Parameters<TaskTransport['subscribe']>[0]) {
    const lines = frame.split('\n')
    const event = lines.find(line => line.startsWith('event: '))?.slice(7)
    const raw = lines.filter(line => line.startsWith('data: ')).map(line => line.slice(6)).join('\n')
    if (!event || !raw) return

    const data: unknown = JSON.parse(raw)
    switch (event) {
        case 'settings-snapshot':
        case 'settings-updated':
            handlers.settings(data as SettingsSnapshot)
            break
        case 'task-snapshot':
            handlers.snapshot(data as TaskRecord[])
            break
        case 'task-updated':
            handlers.updated(data as TaskRecord)
            break
        case 'task-removed':
            handlers.removed(data as string)
            break
        case 'download-metadata-error': {
            const payload = data as { taskId: string; errorMsg: string }
            handlers.metadataError({
                task_id: payload.taskId,
                error_msg: payload.errorMsg,
            } as DownloadMetadataErrorPayload)
            break
        }
    }
}

/** fetch 支持 Authorization 请求头；EventSource 无法带 Bearer 令牌。 */
async function followEvents(signal: AbortSignal, handlers: Parameters<TaskTransport['subscribe']>[0]) {
    let retryDelay = 1000
    while (!signal.aborted) {
        try {
            const response = await fetch('/api/events', {
                headers: webHeaders(),
                cache: 'no-store',
                signal,
            })
            if (response.status === 401) {
                webSession.authorized = false
                return
            }
            if (!response.ok || !response.body) {
                throw new Error(`SSE 连接失败：HTTP ${response.status}`)
            }
            retryDelay = 1000
            const reader = response.body.getReader()
            const decoder = new TextDecoder()
            let buffer = ''
            while (!signal.aborted) {
                const { done, value } = await reader.read()
                if (done) break
                buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, '\n')
                let boundary = buffer.indexOf('\n\n')
                while (boundary >= 0) {
                    const frame = buffer.slice(0, boundary)
                    buffer = buffer.slice(boundary + 2)
                    try {
                        dispatchEvent(frame, handlers)
                    } catch (error) {
                        console.warn('忽略无法解析的任务事件:', error)
                    }
                    boundary = buffer.indexOf('\n\n')
                }
            }
        } catch (error) {
            if (signal.aborted) break
            console.warn('任务事件连接已断开，准备重连:', error)
        }
        // 重连后服务端首先发送完整快照，浏览器离线期间遗漏的事件不会造成状态陈旧。
        await new Promise(resolve => setTimeout(resolve, retryDelay))
        retryDelay = Math.min(retryDelay * 2, 10000)
    }
}

function taskAction(taskId: string, action: string, body?: object): Promise<unknown> {
    return webRequest(`/api/tasks/${encodeURIComponent(taskId)}/${action}`, {
        method: 'POST',
        body: JSON.stringify(body ?? {}),
    })
}

export const webTaskTransport: TaskTransport = {
    list: () => webRequest<TaskRecord[]>('/api/tasks'),
    create: request => webRequest('/api/tasks', {
        method: 'POST',
        body: JSON.stringify(request),
    }),
    async pause(taskId) { await taskAction(taskId, 'pause') },
    async resume(taskId) { await taskAction(taskId, 'resume') },
    async cancel(taskId, deleteFile) {
        await taskAction(taskId, 'cancel', { deleteFile })
    },
    remove: (taskIds, deleteFile) => webRequest('/api/tasks/remove', {
        method: 'POST',
        body: JSON.stringify({ taskIds, deleteFile }),
    }),
    async retry(taskId) {
        const result = await taskAction(taskId, 'retry') as { retried: boolean }
        return result.retried
    },
    async subscribe(handlers) {
        const controller = new AbortController()
        void followEvents(controller.signal, handlers)
        return () => controller.abort()
    },
}

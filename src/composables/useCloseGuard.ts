import { onMounted, onUnmounted } from 'vue'
import { useDialog } from 'naive-ui'
import { subscribeCloseRequest } from '../api/runtimeApi'
import { useTaskStore } from '../stores/taskStore'

export function useCloseGuard() {
    const dialog = useDialog()
    const taskStore = useTaskStore()
    let unlisten: (() => void) | undefined
    let disposed = false

    onMounted(async () => {
        try {
            const cleanup = await subscribeCloseRequest(async () => {
                const activeTasks = taskStore.tasks.filter(task =>
                    task.status === 'waiting' ||
                    task.status === 'downloading' ||
                    task.status === 'paused'
                )

                if (activeTasks.length === 0) {
                    return true
                }

                // 窗口确认仍由 UI 负责；是否拦截原生关闭事件由 API 层处理。
                return new Promise<boolean>(resolve => {
                    dialog.warning({
                        title: '确认退出',
                        content: `有 ${activeTasks.length} 个下载任务尚未完成，退出后任务会中断。确认退出吗？`,
                        positiveText: '确认退出',
                        negativeText: '取消',
                        onPositiveClick: () => resolve(true),
                        onNegativeClick: () => resolve(false),
                        onClose: () => resolve(false),
                    })
                })
            })

            // 若组件在异步注册完成前已卸载，立即撤销迟到的监听器。
            if (disposed) {
                cleanup()
            } else {
                unlisten = cleanup
            }
        } catch (error) {
            console.error('注册窗口关闭监听失败:', error)
        }
    })

    onUnmounted(() => {
        disposed = true
        unlisten?.()
    })
}

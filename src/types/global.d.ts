import type { NotificationApi } from 'naive-ui'

declare global {
    interface Window {
        /** 由 NavLayout 在 Provider 内挂载，供 store 和非组件逻辑显示通知。 */
        $notify?: NotificationApi
    }
}

export {}

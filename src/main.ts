import '@sahil-vartak/tauri-plugin-safe-area-insets-css-api'
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import naive from 'naive-ui'
import App from './App.vue'
import router from './router'
import { useSettingsStore } from './stores/settingsStore'
import { useHistoryStore } from './stores/historyStore'
import { useTaskStore } from './stores/taskStore'
import './style.css'

const app = createApp(App)
const pinia = createPinia()

// 保存任务事件监听的清理函数，用于在应用退出时注销监听器
let cleanupTaskListeners: (() => void) | null = null

app.use(pinia)
app.use(router)
app.use(naive)

async function init() {
    const settingsStore = useSettingsStore()
    const historyStore = useHistoryStore()
    const taskStore = useTaskStore()

    // 先建立任务订阅，再读取快照；读取期间的事件由 store 排队回放。
    try {
        await Promise.all([
            settingsStore.loadSettings(),
            historyStore.loadHistory(),
        ])
    } catch (e) {
        console.error('加载持久化数据失败，使用默认值:', e)
    }

    // 注册任务事件监听；若注册失败，仍继续加载快照并挂载页面。
    try {
        // 保存清理函数，并在页面卸载时调用，避免内存泄漏
        cleanupTaskListeners = await taskStore.setupListeners()
    } catch (e) {
        console.error('注册下载事件监听失败:', e)
    }
    try {
        await taskStore.loadTasks()
    } catch (e) {
        console.error('加载任务失败:', e)
    }
}

init().finally(() => {
    app.mount('#app')

    // 在窗口关闭或刷新前执行清理函数，移除事件监听器
    window.addEventListener('beforeunload', () => {
        if (cleanupTaskListeners) {
            cleanupTaskListeners()
            cleanupTaskListeners = null
        }
    })
})

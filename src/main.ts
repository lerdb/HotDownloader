import { createApp, watch } from 'vue'
import { createPinia } from 'pinia'
import naive from 'naive-ui'
import App from './App.vue'
import router from './router'
import { useSettingsStore } from './stores/settingsStore'
import { useHistoryStore } from './stores/historyStore'
import { useTaskStore } from './stores/taskStore'
import { initializeNativeSafeArea, isNativeRuntime } from './api/runtimeApi'
import { authorizeWeb, webSession } from './api/webClient'
import './style.css'

const app = createApp(App)
const pinia = createPinia()

// 保存任务事件监听的清理函数，用于在应用退出时注销监听器
let cleanupTaskListeners: (() => void) | null = null

app.use(pinia)
app.use(router)
app.use(naive)

async function init() {
    // 原生安全区域只在 Tauri 环境加载，普通浏览器页面无需等待原生插件。
    try {
        await initializeNativeSafeArea()
    } catch (error) {
        console.warn('初始化安全区域失败:', error)
    }
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

    // 重新登录后重新建立订阅，先清理旧连接，避免重复推送同一任务。
    cleanupTaskListeners?.()
    cleanupTaskListeners = null
    // 注册任务事件监听；若注册失败，仍继续加载快照。
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

if (isNativeRuntime()) {
    // 保持原生端原有的初始化顺序，避免窗口内容先于安全区域插件挂载。
    void init().finally(() => app.mount('#app'))
} else {
    // Web 必须先显示令牌输入页，认证后再加载设置和任务投影。
    app.mount('#app')
    // Web 首先验证 API 令牌，再加载设置和任务；认证失败时显示输入页。
    watch(() => webSession.authorized, authorized => {
        if (authorized) void init()
    })
    void authorizeWeb()
}

window.addEventListener('beforeunload', () => {
    cleanupTaskListeners?.()
    cleanupTaskListeners = null
})

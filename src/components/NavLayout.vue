<template>
    <div class="nav-layout" :class="{ 'is-narrow': isNarrow }">
        <!-- 宽屏左侧导航栏（SimpMusic 桌面端侧栏风格） -->
        <aside
            v-if="!isNarrow && !isFullscreen"
            class="sidebar"
            aria-label="主导航"
        >
            <div class="sidebar-brand">
                <div class="brand-badge" aria-hidden="true">
                    <AppIcon name="music" :size="20" />
                </div>
                <div class="brand-text">
                    <span class="brand-title">HotDownloader</span>
                    <span class="brand-subtitle">Hi-Res Music</span>
                </div>
            </div>

            <nav class="sidebar-nav">
                <button
                    v-for="item in navItems"
                    :key="item.key"
                    type="button"
                    class="sidebar-item"
                    :class="{ 'is-active': currentRoute === item.key }"
                    :aria-current="
                        currentRoute === item.key ? 'page' : undefined
                    "
                    @click="handleMenuClick(item.key)"
                >
                    <span class="sidebar-item-icon">
                        <AppIcon :name="item.icon" :size="19" />
                    </span>
                    <span class="sidebar-item-label">{{ item.label }}</span>
                    <span
                        v-if="item.key === '/task' && activeTaskCount > 0"
                        class="nav-badge"
                    >
                        {{ activeTaskCount > 99 ? '99+' : activeTaskCount }}
                    </span>
                </button>
            </nav>

            <div class="sidebar-footer">
                <button
                    type="button"
                    class="theme-cycle-btn"
                    :title="`当前外观：${themeModeLabel}（点击切换）`"
                    @click="cycleThemeMode"
                >
                    <AppIcon :name="themeModeIcon" :size="16" />
                    <span>{{ themeModeLabel }}</span>
                </button>
            </div>
        </aside>

        <div class="content-column">
            <!-- 网页任务进度依赖 SSE；连接中断时保留最后快照并明确标记其时间。 -->
            <div
                v-if="!native && !isFullscreen"
                class="connection-strip"
                :class="taskStore.connectionStatus"
            >
                <span class="connection-dot" aria-hidden="true"></span>
                <span>{{ connectionLabel }}</span>
                <span class="last-response"
                    >最近响应：{{ lastResponseLabel }}</span
                >
            </div>

            <div
                v-if="
                    !isFullscreen &&
                    (taskStore.batchSubmission.running ||
                        taskStore.queueStatus.paused)
                "
                class="status-banners"
            >
                <n-alert
                    v-if="taskStore.batchSubmission.running"
                    type="info"
                    title="正在提交批量任务"
                    class="status-banner"
                >
                    <div class="banner-row">
                        <span>
                            已处理 {{ taskStore.batchSubmission.processed }} /
                            {{ taskStore.batchSubmission.total }} 首
                        </span>
                        <n-button
                            size="small"
                            :disabled="taskStore.batchSubmission.stopping"
                            @click="taskStore.stopSubmission()"
                        >
                            {{
                                taskStore.batchSubmission.stopping
                                    ? '正在停止…'
                                    : '停止后续提交'
                            }}
                        </n-button>
                    </div>
                </n-alert>
                <n-alert
                    v-if="taskStore.queueStatus.paused"
                    type="warning"
                    title="下载队列已暂停"
                    class="status-banner"
                >
                    <div class="banner-row">
                        <span>{{ taskStore.queueStatus.reason }}</span>
                        <n-button size="small" @click="resumeQueue"
                            >恢复队列</n-button
                        >
                    </div>
                </n-alert>
            </div>

            <!-- 内容区域 -->
            <main
                ref="mainContentRef"
                class="main-content"
                :class="{
                    'has-bottom-nav': isNarrow && !isFullscreen,
                    'is-fullscreen': isFullscreen,
                }"
            >
                <div
                    class="main-content-inner"
                    :class="{ 'is-fullscreen': isFullscreen }"
                >
                    <router-view v-slot="{ Component }">
                        <!-- 每个详情独立缓存，返回时恢复其分页、标签与勾选。 -->
                        <keep-alive :max="MAX_CACHED_VIEWS">
                            <component
                                v-if="route.meta.keepAlive === true"
                                :is="Component"
                                :key="viewKey"
                            />
                        </keep-alive>
                        <component
                            v-if="route.meta.keepAlive !== true"
                            :is="Component"
                            :key="viewKey"
                        />
                    </router-view>
                </div>
            </main>

            <PlaybackBar v-if="!isFullscreen" />
        </div>

        <!-- 窄屏底部水平导航：毛玻璃质感，正常流固定占位 -->
        <footer v-if="isNarrow && !isFullscreen" class="bottom-nav">
            <nav class="bottom-nav-inner" aria-label="底部导航">
                <button
                    v-for="item in navItems"
                    :key="item.key"
                    type="button"
                    class="bottom-nav-item"
                    :class="{ 'is-active': currentRoute === item.key }"
                    :aria-current="
                        currentRoute === item.key ? 'page' : undefined
                    "
                    @click="handleMenuClick(item.key)"
                >
                    <span class="bottom-nav-pill">
                        <AppIcon :name="item.icon" :size="20" />
                        <span
                            v-if="item.key === '/task' && activeTaskCount > 0"
                            class="bottom-badge"
                            aria-hidden="true"
                        />
                    </span>
                    <span class="bottom-nav-label">{{ item.label }}</span>
                </button>
            </nav>
        </footer>
    </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { NAlert, NButton, useNotification } from 'naive-ui'
import { useCloseGuard } from '../composables/useCloseGuard'
import { useNarrowLayout } from '../composables/useNarrowLayout'
import { useAppTheme } from '../composables/useAppTheme'
import { isNativeRuntime } from '../api/runtimeApi'
import { showStorageRecoveryReport } from '../api/storageRecovery'
import { useTaskStore } from '../stores/taskStore'
import PlaybackBar from './PlaybackBar.vue'
import AppIcon, { type AppIconName } from './common/AppIcon.vue'
import { usePlayerStore } from '../stores/playerStore'
import { useAccountStore } from '../stores/accountStore'
import {
    MAX_CACHED_VIEWS,
    createViewScrollCache,
    viewCacheKey,
} from '../utils/viewCache'

const router = useRouter()
const route = useRoute()
const native = isNativeRuntime()
const taskStore = useTaskStore()
const { themeMode, setThemeMode } = useAppTheme()

async function resumeQueue() {
    try {
        await taskStore.controlQueue('resume')
    } catch (error) {
        window.$notify?.error({
            title: '恢复队列失败',
            description: String(error),
        })
    }
}

const player = usePlayerStore()
const connectionLabel = computed(
    () =>
        ({
            connecting: '正在连接服务',
            connected: '服务已连接',
            reconnecting: '连接中断，正在重连',
            disconnected: '服务连接已断开',
        })[taskStore.connectionStatus],
)
const lastResponseLabel = computed(() =>
    taskStore.lastServerActivityAt
        ? new Date(taskStore.lastServerActivityAt).toLocaleTimeString()
        : '尚未收到响应',
)
const viewKey = computed(() => viewCacheKey(route))

const activeTaskCount = computed(
    () =>
        taskStore.tasks.filter((task) =>
            ['waiting', 'downloading', 'processing'].includes(task.status),
        ).length,
)

const themeModeLabel = computed(
    () =>
        ({
            system: '跟随系统',
            dark: '深色模式',
            light: '浅色模式',
        })[themeMode.value],
)

const themeModeIcon = computed<AppIconName>(
    () =>
        ({
            system: 'monitor',
            dark: 'moon',
            light: 'sun',
        })[themeMode.value] as AppIconName,
)

function cycleThemeMode() {
    const order: Array<'system' | 'dark' | 'light'> = ['system', 'dark', 'light']
    const next = order[(order.indexOf(themeMode.value) + 1) % order.length]
    setThemeMode(next)
}

// 保存各路由页面的滚动位置，实现独立滚动记录
const mainContentRef = ref<HTMLElement | null>(null)
const scrollPositions = createViewScrollCache()
let scrollRequest = 0

let removeRouteGuard: (() => void) | null = null

// 恢复指定路由的滚动位置
async function restoreScrollPosition(path: string) {
    const current = ++scrollRequest
    const position = scrollPositions.activate(route)
    await nextTick()
    if (
        mainContentRef.value &&
        current === scrollRequest &&
        route.fullPath === path
    ) {
        mainContentRef.value.scrollTop = position
    }
}

// 监听路由变化，恢复新路由的滚动位置
watch(
    () => route.fullPath,
    (newPath) => {
        restoreScrollPosition(newPath)
    },
)

// 在 n-dialog-provider 内部调用，确保 useDialog 正常工作
useCloseGuard()

// 挂载通知实例到全局，供 store 使用
const notification = useNotification()
window.$notify = notification
const accounts = useAccountStore()
// 原生启动检查可能先于通知容器完成；由应用布局消费待提示消息，避免首屏丢提示。
watch(
    () => accounts.startupWarnings.slice(),
    (messages) => {
        if (!messages.length) return
        for (const description of accounts.takeStartupWarnings())
            notification.warning({
                title: '账号登录需处理',
                description,
                duration: 8000,
            })
    },
    { immediate: true },
)

// 移动端响应式布局状态；公共方法统一断点，并在组件销毁时清理监听。
const isNarrow = useNarrowLayout()
const isFullscreen = computed(() => route.meta.fullscreen === true)

onMounted(() => {
    if (native) void showStorageRecoveryReport()
    // 注册全局前置守卫，在离开当前路由前保存滚动位置
    removeRouteGuard = router.beforeEach((_to, from) => {
        if (mainContentRef.value) {
            scrollPositions.remember(from, mainContentRef.value.scrollTop)
        }
    })
    // 初始恢复当前路由的滚动位置（如果有保存过）
    restoreScrollPosition(route.fullPath)
})

onUnmounted(() => {
    player.stop()
    // 移除路由守卫，避免内存泄漏
    if (removeRouteGuard) {
        removeRouteGuard()
        removeRouteGuard = null
    }
})

// 关于页属于设置入口，返回时继续保持设置菜单高亮。
const currentRoute = computed(() => {
    if (route.path.startsWith('/playlist')) return '/playlist'
    if (route.path === '/album' || route.path === '/artist') return '/search'
    return route.path.startsWith('/settings/') ? '/settings' : route.path
})

const navItems: { label: string; key: string; icon: AppIconName }[] = [
    { label: '搜索', key: '/search', icon: 'search' },
    { label: '歌单', key: '/playlist', icon: 'playlist' },
    { label: '任务', key: '/task', icon: 'task' },
    { label: '设置', key: '/settings', icon: 'settings' },
]

function handleMenuClick(key: string) {
    if (key !== route.path) {
        router.push(key)
    }
}
</script>

<style scoped>
/* 布局整体 */
.nav-layout {
    --page-padding: 28px;

    display: flex;
    height: 100%;
    min-height: 0;
    background-color: var(--bg-body);
}

.nav-layout.is-narrow {
    --page-padding: 16px;

    flex-direction: column;
}

.content-column {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    position: relative;
}

.connection-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    padding: 6px calc(var(--page-padding) + var(--safe-area-right));
    background: var(--bg-glass);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    border-bottom: 1px solid var(--border-color);
    font-size: 12px;
    color: var(--color-text-secondary);
}

.connection-dot {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
    background: #d99335;
}

.connection-strip.connected .connection-dot {
    background: #18a058;
    box-shadow: 0 0 8px rgba(24, 160, 88, 0.55);
}

.connection-strip.disconnected .connection-dot {
    background: #d03050;
}

.last-response {
    margin-left: auto;
    text-align: right;
}

.status-banners {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px calc(var(--page-padding) + var(--safe-area-right)) 0
        calc(var(--page-padding) + var(--safe-area-left));
    flex-shrink: 0;
}

.status-banner {
    border-radius: 12px;
}

.banner-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
}

/* 桌面侧边栏：SimpMusic 风格分组栏 */
.sidebar {
    width: 224px;
    flex-shrink: 0;
    border-right: 1px solid var(--border-color);
    padding: 20px 14px 16px;
    padding-top: calc(20px + var(--safe-area-top));
    padding-left: calc(14px + var(--safe-area-left));
    background-color: var(--bg-sidebar);
    display: flex;
    flex-direction: column;
    gap: 20px;
    overflow-y: auto;
}

.sidebar-brand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 4px 8px;
}

.brand-badge {
    width: 38px;
    height: 38px;
    border-radius: 12px;
    background: linear-gradient(
        135deg,
        var(--color-primary),
        var(--color-primary-hover)
    );
    color: #ffffff;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    box-shadow: 0 6px 16px var(--color-primary-soft);
}

.brand-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
}

.brand-title {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.01em;
    color: var(--color-text);
    line-height: 1.25;
}

.brand-subtitle {
    font-size: 11px;
    color: var(--color-text-secondary);
    letter-spacing: 0.02em;
}

.sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
}

.sidebar-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 14px;
    border: none;
    border-radius: 12px;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition:
        background-color 0.18s ease,
        color 0.18s ease,
        transform 0.15s ease;
}

.sidebar-item:hover {
    background: var(--bg-surface-subtle);
    color: var(--color-text);
}

.sidebar-item.is-active {
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-weight: 600;
}

.sidebar-item:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 2px;
}

.sidebar-item-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
}

.sidebar-item-label {
    flex: 1;
    min-width: 0;
}

.nav-badge {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--color-primary);
    color: #ffffff;
    font-size: 11px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-variant-numeric: tabular-nums;
}

:root[data-theme='dark'] .nav-badge,
:root[data-theme='dark'] .brand-badge {
    color: #0b1210;
}

.sidebar-footer {
    padding-top: 12px;
    border-top: 1px solid var(--border-color);
}

.theme-cycle-btn {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 12px;
    border: 1px solid var(--border-color);
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition:
        color 0.18s ease,
        border-color 0.18s ease,
        background-color 0.18s ease;
}

.theme-cycle-btn:hover {
    color: var(--color-text);
    border-color: var(--border-color-strong);
}

/* 主内容区：顶部柔和主题光晕 */
.main-content {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    background-color: var(--bg-content);
    background-image: radial-gradient(
        ellipse 80% 240px at 50% -40px,
        var(--color-primary-soft),
        transparent 70%
    );

    /* 用四个方向分别声明，让左右两侧都避开安全区 */
    padding: var(--page-padding);
    padding-left: calc(var(--page-padding) + var(--safe-area-left));
    padding-right: calc(var(--page-padding) + var(--safe-area-right));

    /* 将回弹限制在当前滚动容器内部，保留视觉回弹但阻断滚动链向上传播 */
    overscroll-behavior: contain;
}

.main-content.is-fullscreen {
    padding: 0;
    overflow: hidden;
    background-image: none;
}

.main-content-inner {
    width: 100%;
    max-width: 1120px;
    min-width: 0;
    min-height: 100%;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
}

.main-content-inner.is-fullscreen {
    max-width: none;
    height: 100%;
}

/* 为正常流底部导航保留合适的底部间距，避免内容与导航粘连 */
.main-content.has-bottom-nav {
    padding-bottom: 18px;
    padding-left: calc(16px + var(--safe-area-left));
    padding-right: calc(16px + var(--safe-area-right));
}

/* 底部导航：SimpMusic 风格毛玻璃导航栏，正常流布局 */
.bottom-nav {
    width: 100%;
    height: 62px;
    flex-shrink: 0;
    border-top: 1px solid var(--border-color);
    background: var(--bg-glass-strong);
    backdrop-filter: blur(22px) saturate(165%);
    -webkit-backdrop-filter: blur(22px) saturate(165%);
    padding-left: var(--safe-area-left);
    padding-right: var(--safe-area-right);
}

.bottom-nav-inner {
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: space-around;
    align-items: center;
    padding: 0 6px;
}

.bottom-nav-item {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    cursor: pointer;
    padding: 4px 2px;
    -webkit-tap-highlight-color: transparent;
    transition: color 0.18s ease;
}

.bottom-nav-pill {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 54px;
    height: 30px;
    border-radius: 999px;
    transition:
        background-color 0.2s ease,
        transform 0.15s ease;
}

.bottom-nav-item.is-active {
    color: var(--color-primary);
}

.bottom-nav-item.is-active .bottom-nav-pill {
    background: var(--color-primary-soft);
}

.bottom-nav-label {
    font-size: 11px;
    font-weight: 500;
    line-height: 1.2;
}

.bottom-nav-item.is-active .bottom-nav-label {
    font-weight: 600;
}

.bottom-badge {
    position: absolute;
    top: 4px;
    right: 12px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--color-primary);
}
</style>

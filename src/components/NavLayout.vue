<template>
    <div class="nav-layout" :class="{ 'is-narrow': isNarrow }">
        <!-- 宽屏左侧垂直导航 -->
        <aside v-if="!isNarrow" class="sidebar">
            <n-menu :value="currentRoute" :options="menuOptions" @update:value="handleMenuClick" />
        </aside>

        <div class="content-column">
            <!-- 网页任务进度依赖 SSE；连接中断时保留最后快照并明确标记其时间。 -->
            <div v-if="!native" class="connection-strip" :class="taskStore.connectionStatus">
                <span class="connection-dot" aria-hidden="true"></span>
                <span>{{ connectionLabel }}</span>
                <span class="last-response">最近响应：{{ lastResponseLabel }}</span>
            </div>

            <!-- 内容区域 -->
            <main ref="mainContentRef" class="main-content" :class="{ 'has-bottom-nav': isNarrow }">
                <router-view v-slot="{ Component }">
                    <!-- 每个详情独立缓存，返回时恢复其分页、标签与勾选。 -->
                    <keep-alive>
                        <component :is="Component" :key="viewKey" />
                    </keep-alive>
                </router-view>
            </main>
        </div>

        <!-- 窄屏底部水平导航：在正常文档流中固定占位，菜单始终居中 -->
        <footer v-if="isNarrow" class="bottom-nav">
            <div class="bottom-nav-inner">
                <n-menu :value="currentRoute" :options="menuOptions" mode="horizontal"
                    @update:value="handleMenuClick" />
            </div>
        </footer>
    </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { NMenu, useNotification, type MenuOption } from 'naive-ui'
import { useCloseGuard } from '../composables/useCloseGuard'
import { useNarrowLayout } from '../composables/useNarrowLayout'
import { isNativeRuntime } from '../api/runtimeApi'
import { useTaskStore } from '../stores/taskStore'

const router = useRouter()
const route = useRoute()
const native = isNativeRuntime()
const taskStore = useTaskStore()
const connectionLabel = computed(() => ({
    connecting: '正在连接服务',
    connected: '服务已连接',
    reconnecting: '连接中断，正在重连',
    disconnected: '服务连接已断开',
})[taskStore.connectionStatus])
const lastResponseLabel = computed(() => taskStore.lastServerActivityAt
    ? new Date(taskStore.lastServerActivityAt).toLocaleTimeString()
    : '尚未收到响应')
const viewKey = computed(() => {
    return ['/artist', '/album'].includes(route.path) ? route.fullPath : route.path
})

// 保存各路由页面的滚动位置，实现独立滚动记录
const mainContentRef = ref<HTMLElement | null>(null)
const scrollPositions: Record<string, number> = {}

let removeRouteGuard: (() => void) | null = null

// 恢复指定路由的滚动位置
async function restoreScrollPosition(path: string) {
    await nextTick()
    if (mainContentRef.value) {
        mainContentRef.value.scrollTop = scrollPositions[path] ?? 0
    }
}

// 监听路由变化，恢复新路由的滚动位置
watch(() => route.fullPath, (newPath) => {
    restoreScrollPosition(newPath)
})

// 在 n-dialog-provider 内部调用，确保 useDialog 正常工作
useCloseGuard()

// 挂载通知实例到全局，供 store 使用
const notification = useNotification()
window.$notify = notification

// 移动端响应式布局状态；公共方法统一断点，并在组件销毁时清理监听。
const isNarrow = useNarrowLayout()

onMounted(() => {
    // 注册全局前置守卫，在离开当前路由前保存滚动位置
    removeRouteGuard = router.beforeEach((_to, from) => {
        if (mainContentRef.value) {
            scrollPositions[from.fullPath] = mainContentRef.value.scrollTop
        }
    })
    // 初始恢复当前路由的滚动位置（如果有保存过）
    restoreScrollPosition(route.fullPath)
})

onUnmounted(() => {
    // 移除路由守卫，避免内存泄漏
    if (removeRouteGuard) {
        removeRouteGuard()
        removeRouteGuard = null
    }
})

// 关于页属于设置入口，返回时继续保持设置菜单高亮。
const currentRoute = computed(() => {
    if (route.path === '/album' || route.path === '/artist') return '/search'
    return route.path.startsWith('/settings/') ? '/settings' : route.path
})

const menuOptions: MenuOption[] = [
    {
        label: '搜索',
        key: '/search'
    },
    {
        label: '歌单',
        key: '/playlist'
    },
    {
        label: '任务',
        key: '/task'
    },
    {
        label: '设置',
        key: '/settings'
    },
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
    --page-padding: 24px;

    display: flex;
    height: 100%;
    min-height: 0;
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
}

.connection-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    padding: 5px calc(var(--page-padding) + var(--safe-area-right));
    background: var(--bg-sidebar);
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
}

.connection-strip.disconnected .connection-dot {
    background: #d03050;
}

.last-response {
    margin-left: auto;
    text-align: right;
}

/* 侧边栏：使用自定义背景变量 */
.sidebar {
    width: 176px;
    flex-shrink: 0;
    border-right: 1px solid var(--border-color);
    padding: 16px 8px;
    background-color: var(--bg-sidebar);
    overflow-y: auto;

    /* 横屏时让导航避开状态栏和侧边安全区。 */
    padding-top: calc(16px + var(--safe-area-top));
    padding-left: calc(8px + var(--safe-area-left));
}

/* 主内容区背景 */
.main-content {
    flex: 1;
    /* 允许内容随窗口收缩，避免长列表撑开整个布局。 */
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    background-color: var(--bg-content);

    /* 用四个方向分别声明，让左右两侧都避开安全区 */
    padding: var(--page-padding);
    padding-left: calc(var(--page-padding) + var(--safe-area-left));
    padding-right: calc(var(--page-padding) + var(--safe-area-right));

    /* 将回弹限制在当前滚动容器内部，保留视觉回弹但阻断滚动链向上传播，恢复主内容区滚动到顶端/底端时的回弹效果，同时避免回弹传播导致底部导航移动 */
    overscroll-behavior: contain;
}

/* 为正常流底部导航保留合适的底部间距，避免内容与导航粘连 */
.main-content.has-bottom-nav {
    padding-bottom: 16px;
    padding-left: calc(16px + var(--safe-area-left));
    padding-right: calc(16px + var(--safe-area-right));
}

/* 底部导航：改用正常流布局（非 fixed），解决 Android 滚动回弹时导航被拉伸的问题 */
/* 需配合 flex 列容器使用，主内容无需预留底部内边距，高度由 flex 分配 */
.bottom-nav {
    width: 100%;
    height: 56px;
    flex-shrink: 0;
    border-top: 1px solid var(--border-color);
    background-color: var(--bg-bottom);
    padding-left: var(--safe-area-left);
    padding-right: var(--safe-area-right);
}

/* 居中容器 */
.bottom-nav-inner {
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
}

/* 穿透样式强制菜单项居中 */
.bottom-nav-inner :deep(.n-menu) {
    width: 100%;
    justify-content: center;
}

.bottom-nav-inner :deep(.n-menu .n-menu-item) {
    flex: 1;
    min-width: 0;
}

/* 四个入口平分底栏，保持文字居中和足够的触控高度。 */
.bottom-nav-inner :deep(.n-menu-item-content) {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 44px;
    padding: 0 8px;
}
</style>

<template>
    <div class="task-view">
        <TaskTabs v-model:activeTab="activeTab" :counts="tabCounts" />

        <!-- 错误任务较多时逐个点太麻烦，提供一键重试（并发仍由后端调度器按最大并发数控制） -->
        <div v-if="activeTab === 'error' && tabCounts.error > 0" class="task-toolbar">
            <n-button size="small" type="primary" :loading="retryingAll" :disabled="retryingAll"
                @click="handleRetryAll">
                全部重试（{{ tabCounts.error }}）
            </n-button>
            <span class="task-toolbar-hint">会依次重新入队，实际同时下载数量由“最大并发数”决定</span>
        </div>

        <TaskTable :tasks="pagedTasks" :selectedRowKeys="selectedRowKeys"
            @update:selectedRowKeys="selectedRowKeys = $event" @action="handleAction" />

        <!-- 任务数量可能很大，只渲染当前页，避免一次性创建成千上万个 DOM/组件导致卡死 -->
        <div v-if="filteredTasks.length > pageSize" class="task-pagination">
            <n-pagination v-model:page="page" :page-size="pageSize" :item-count="filteredTasks.length"
                :page-slot="5" />
        </div>

        <TaskBatchActions :selectedCount="selectedRowKeys.length" @clear="handleBatchClear" />
    </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NPagination, NButton, useNotification } from 'naive-ui'
import { useTaskStore } from '../stores/taskStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import TaskTabs from '../components/task/TaskTabs.vue'
import TaskTable from '../components/task/TaskTable.vue'
import TaskBatchActions from '../components/task/TaskBatchActions.vue'

const taskStore = useTaskStore()
const { retryTask } = useDownloadActions()
const notification = useNotification()

const activeTab = ref('all')
const selectedRowKeys = ref<string[]>([])
const retryingAll = ref(false)

// 分页：任务列表可能包含上千条记录（尤其是“全部/已完成”），
// 一次渲染全部任务会创建大量组件实例并频繁重渲染，是崩溃与卡顿的主因之一。
const page = ref(1)
const pageSize = ref(50)

const tabCounts = computed(() => {
    const counts = {
        total: 0,
        waiting: 0,
        downloading: 0,
        paused: 0,
        completed: 0,
        error: 0,
    }
    for (const task of taskStore.tasks) {
        counts.total++
        if (task.status === 'waiting') counts.waiting++
        else if (task.status === 'downloading') counts.downloading++
        else if (task.status === 'paused') counts.paused++
        else if (task.status === 'completed') counts.completed++
        else if (task.status === 'error') counts.error++
    }
    return counts
})

const filteredTasks = computed(() => {
    const tab = activeTab.value
    return taskStore.tasks.filter((t) => {
        // 显式读取进度相关字段，建立响应式依赖
        void t.downloaded;
        void t.fileSize;
        return tab === 'all' || t.status === tab;
    });
})

const pagedTasks = computed(() => {
    const start = (page.value - 1) * pageSize.value
    return filteredTasks.value.slice(start, start + pageSize.value)
})

// 切换标签页时回到第一页
watch(activeTab, () => {
    page.value = 1
})

// 任务被删除或筛选结果变少时，纠正越界页码，避免停留在空白页
watch(
    () => filteredTasks.value.length,
    (len) => {
        const maxPage = Math.max(1, Math.ceil(len / pageSize.value))
        if (page.value > maxPage) page.value = maxPage
    }
)

async function handleAction(action: string, taskId: string, extra?: any) {
    switch (action) {
        case 'cancel':
            taskStore.cancelTask(taskId, extra?.deleteFile === true)
            break
        case 'pause':
            taskStore.pauseTask(taskId)
            break
        case 'resume':
            taskStore.resumeTask(taskId)
            break
        case 'retry':
            await retryTask(taskId)
            break
        case 'remove':
            await taskStore.removeTask(taskId, extra?.deleteFile === true)
            break
        case 'open-location': {
            const task = taskStore.tasks.find((t) => t.id === taskId)
            if (task?.filePath) {
                try {
                    await invoke('open_file_location', { path: task.filePath })
                } catch (e) {
                    console.error('打开文件位置失败:', e)
                }
            }
            break
        }
    }
    // 清除相关选中状态
    selectedRowKeys.value = selectedRowKeys.value.filter((id) => id !== taskId)
}

async function handleBatchClear(deleteFile: boolean) {
    const ids = selectedRowKeys.value.slice()
    selectedRowKeys.value = []
    if (ids.length === 0) return
    // 一次性提交给后端批量删除，前端只落盘一次（旧实现是逐个任务 invoke + 逐个整表写盘）
    await taskStore.removeTasks(ids, deleteFile)
}

/** 一键重试当前所有“错误”状态的任务 */
async function handleRetryAll() {
    if (retryingAll.value) return
    const ids = taskStore.tasks
        .filter((t) => t.status === 'error')
        .map((t) => t.id)
    if (ids.length === 0) return

    retryingAll.value = true
    try {
        const { succeeded, failed } = await taskStore.retryTasks(ids)
        notification.success({
            title: '批量重试',
            description: `已重新入队 ${succeeded} 个任务${failed > 0 ? `，${failed} 个无法重试（重试次数用尽或无可降级音质）` : ''}`,
            duration: 4000,
        })
    } catch (e: any) {
        console.error('批量重试失败:', e)
        notification.error({ title: '批量重试失败', description: e?.message || String(e), duration: 4000 })
    } finally {
        retryingAll.value = false
    }
}
</script>

<style scoped>
.task-view {
    display: flex;
    flex-direction: column;
    height: 100%;
}

.task-pagination {
    display: flex;
    justify-content: center;
    padding: 12px 0 0;
}

.task-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    flex-wrap: wrap;
}

.task-toolbar-hint {
    font-size: 12px;
    color: var(--n-text-color-3, #999);
}
</style>

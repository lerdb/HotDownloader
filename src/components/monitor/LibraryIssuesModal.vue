<template>
    <n-modal
        v-model:show="show"
        preset="card"
        title="音乐库异常文件"
        class="monitor-modal"
    >
        <p class="muted">
            修复文件或标签后，点击音乐库的“立即扫描”，再对待确认歌曲刷新候选。
        </p>
        <n-space align="center">
            <n-input
                v-model:value="query"
                clearable
                placeholder="搜索文件路径"
            />
            <n-button :loading="loading" @click="load">刷新列表</n-button>
        </n-space>
        <n-alert v-if="error" type="error">{{ error }}</n-alert>
        <p>共 {{ total }} 个异常文件</p>
        <n-empty v-if="!loading && !files.length" description="暂无异常文件" />
        <article v-for="file in files" :key="file.path" class="issue-file">
            <code>{{ file.path }}</code>
            <p v-if="file.warning">{{ file.warning }}</p>
            <p v-if="file.conflict">音频标签与文件名冲突</p>
            <p v-if="!file.identity.title || !file.identity.artists.length">
                缺少可识别的标题或歌手
            </p>
            <p class="muted">
                标签：{{ file.metadata.title || '缺失' }} ·
                {{ file.metadata.artists.join('、') || '缺失' }}
            </p>
            <p class="muted">
                文件名：{{ file.filename.title || '未解析' }} ·
                {{ file.filename.artists.join('、') || '未解析' }}
            </p>
        </article>
        <n-pagination v-model:page="page" :page-size="30" :item-count="total" />
    </n-modal>
</template>
<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import {
    NModal,
    NSpace,
    NInput,
    NButton,
    NAlert,
    NEmpty,
    NPagination,
} from 'naive-ui'
import { getLibraryIssues, type LocalFile } from '../../api/monitorApi'
import { monitorErrorMessage } from './presentation'
const show = defineModel<boolean>('show', { required: true })
const page = ref(1),
    total = ref(0),
    query = ref(''),
    error = ref(''),
    loading = ref(false)
const files = ref<LocalFile[]>([])
let generation = 0
let timer: ReturnType<typeof setTimeout> | undefined
async function load() {
    if (!show.value) return
    const request = ++generation
    loading.value = true
    try {
        const result = await getLibraryIssues(page.value, query.value)
        if (request !== generation || !show.value) return
        files.value = result.items
        total.value = result.total
        page.value = result.page
        error.value = ''
    } catch (e) {
        if (request === generation) error.value = monitorErrorMessage(e)
    } finally {
        if (request === generation) loading.value = false
    }
}
watch(show, (value) => {
    generation++
    clearTimeout(timer)
    if (value) {
        page.value = 1
        void load()
    }
})
watch(page, () => void load())
watch(query, () => {
    generation++
    clearTimeout(timer)
    page.value = 1
    timer = setTimeout(() => void load(), 300)
})
onUnmounted(() => {
    generation++
    clearTimeout(timer)
})
</script>
<style scoped src="./monitor-shared.css"></style>
<style scoped>
.issue-file {
    border-bottom: 1px solid var(--border-color);
    padding: 12px 0;
    overflow-wrap: anywhere;
}
</style>

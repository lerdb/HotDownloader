<template>
    <div class="monitor-page">
        <div class="toolbar">
            <n-button @click="router.push('/playlist')">返回歌单</n-button>
            <h2>歌单监控</h2>
            <n-button type="primary" @click="edit()">添加监控</n-button>
        </div>
        <n-alert v-if="error" type="error" title="操作未完成">{{
            error
        }}</n-alert>
        <n-alert v-if="persistenceFailed" type="error"
            >处理台账保存失败，自动下载已停止。请检查服务存储并重启。</n-alert
        >
        <n-card title="本地音乐库" size="small">
            <template #header-extra
                ><n-button
                    :loading="scanning || library?.scanning"
                    :disabled="busy"
                    @click="scan"
                    >立即扫描</n-button
                ></template
            >
            <template v-if="library">
                <p>
                    已索引 {{ library.fileCount }} 个文件 · 最近完整扫描：{{
                        time(library.lastSuccess)
                    }}
                </p>
                <n-alert
                    v-if="library.error"
                    type="warning"
                    title="本轮补齐已跳过"
                    >{{ library.error }}。保留上次完整索引。</n-alert
                >
                <p v-if="library.unresolvedCount" class="muted">
                    {{ library.unresolvedCount }}
                    个文件信息不完整，无法可靠匹配的歌曲会进入待确认。
                </p>
                <div
                    v-for="root in library.roots"
                    :key="root.path"
                    class="root-row"
                >
                    <code>{{ root.path }}</code>
                    <span class="muted"
                        >模板：{{ root.template || '仅使用音频标签' }} ·
                        歌手分隔符：{{ root.artistSeparator }}</span
                    >
                </div>
                <p class="muted">
                    目录由服务端环境变量配置；下载目录自动扫描，其他 NAS
                    目录请只读挂载。
                </p>
            </template>
        </n-card>

        <p class="muted">
            首次补齐当前缺失歌曲，之后只处理新增歌曲。处理决定按 MID
            跨歌单共享。{{ running ? '服务正在检查或补齐…' : '' }}
        </p>
        <n-empty
            v-if="!monitors.length"
            description="添加 QQ 歌单或“我喜欢”，开始自动补齐"
        />
        <n-card
            v-for="monitor in monitors"
            :key="monitor.id"
            :title="monitor.name"
            size="small"
        >
            <template #header-extra
                ><n-switch
                    :value="monitor.enabled"
                    :disabled="busy"
                    @update:value="toggle(monitor, $event)"
                    ><template #checked>已启用</template
                    ><template #unchecked>已停用</template></n-switch
                ></template
            >
            <p>
                {{ sourceLabel(monitor.source) }} · {{ monitor.quality }} · 每
                {{ monitor.intervalMinutes }} 分钟
            </p>
            <p class="muted">
                上次：{{ time(monitor.lastCheck) }} · 下次：{{
                    monitor.enabled
                        ? time(monitor.nextCheck, '尽快检查')
                        : '已停用'
                }}
            </p>
            <p>{{ monitor.lastResult }}</p>
            <n-space
                ><n-tag
                    v-for="(count, state) in monitor.counts"
                    :key="state"
                    :type="stateType(String(state))"
                    >{{ stateLabel(String(state)) }} {{ count }}</n-tag
                ></n-space
            >
            <n-space class="actions">
                <n-button :disabled="busy" @click="check(monitor)"
                    >立即检查</n-button
                >
                <n-button :disabled="busy" @click="edit(monitor)"
                    >编辑</n-button
                >
                <n-button
                    :type="selectedId === monitor.id ? 'primary' : 'default'"
                    @click="selectMonitor(monitor.id)"
                    >查看歌曲</n-button
                >
            </n-space>
        </n-card>

        <n-card
            v-if="selectedId"
            :title="`${selectedName} · 逐曲状态`"
            size="small"
        >
            <div class="song-filters">
                <n-select
                    v-model:value="filter"
                    :options="filterOptions"
                /><n-input
                    v-model:value="query"
                    clearable
                    placeholder="搜索标题或歌手"
                />
            </div>
            <p class="muted">
                共
                {{ filteredSongs.length }}
                首。忽略或关联后，即使文件移动或任务记录清除，也不会自动重下。
            </p>
            <n-empty
                v-if="!filteredSongs.length"
                description="暂无符合条件的歌曲"
            />
            <article
                v-for="entry in pageSongs"
                :key="entry.song.mid"
                class="song-row"
            >
                <div class="song-header">
                    <strong>{{ entry.song.title }}</strong
                    ><n-tag :type="stateType(entry.state)">{{
                        stateLabel(entry.state)
                    }}</n-tag>
                </div>
                <p>
                    {{ entry.song.artist }}
                    <span class="muted">· {{ entry.quality }}</span>
                </p>
                <p class="muted">{{ entry.message }}</p>
                <code v-if="entry.path">{{ entry.path }}</code>
                <p
                    v-if="
                        ['download_failed', 'credential_invalid'].includes(
                            entry.state,
                        )
                    "
                    class="muted"
                >
                    已自动重试 {{ entry.retries }} / 3 次 ·
                    {{
                        entry.nextRetry
                            ? `下次重试：${time(entry.nextRetry)}`
                            : '已停止自动重试'
                    }}
                </p>
                <n-space class="actions">
                    <template v-if="entry.state === 'pending_confirmation'">
                        <n-button :disabled="busy" @click="openLink(entry)"
                            >关联现有文件</n-button
                        >
                        <n-button
                            :disabled="busy"
                            @click="decide(entry, 'download')"
                            >下载</n-button
                        >
                        <n-button
                            :disabled="busy"
                            @click="decide(entry, 'ignore')"
                            >忽略</n-button
                        >
                    </template>
                    <n-button v-if="entry.taskId" @click="router.push('/task')"
                        >查看任务</n-button
                    >
                    <n-button
                        v-if="entry.state === 'credential_invalid'"
                        @click="router.push('/settings')"
                        >前往登录</n-button
                    >
                    <n-popconfirm
                        v-if="canReset(entry.state)"
                        @positive-click="decide(entry, 'reset')"
                    >
                        <template #trigger
                            ><n-button :disabled="busy"
                                >清除决定并重新下载</n-button
                            ></template
                        >
                        将清除此歌曲在所有监控歌单中的处理决定，并重新下载；已有文件保留。
                    </n-popconfirm>
                </n-space>
            </article>
            <n-pagination
                v-model:page="page"
                :page-size="30"
                :item-count="filteredSongs.length"
            />
        </n-card>

        <n-modal
            v-model:show="showEditor"
            preset="card"
            :title="editingId ? '编辑监控' : '添加监控'"
            class="monitor-modal"
            style="width: min(480px, calc(100vw - 32px))"
        >
            <n-form label-placement="top">
                <n-form-item label="名称"
                    ><n-input
                        v-model:value="form.name"
                        maxlength="200"
                        placeholder="例如：通勤歌单"
                /></n-form-item>
                <n-form-item label="来源"
                    ><n-select
                        v-model:value="form.source"
                        :options="sourceOptions"
                        :disabled="!!editingId"
                /></n-form-item>
                <n-form-item
                    v-if="form.source === 'created' && !editingId"
                    label="我的 QQ 歌单"
                >
                    <n-space vertical style="width: 100%"
                        ><n-button
                            :loading="loadingPlaylists"
                            @click="loadPlaylists"
                            >读取我的歌单</n-button
                        ><n-select
                            :options="playlistOptions"
                            placeholder="选择个人歌单"
                            @update:value="choosePlaylist"
                    /></n-space>
                </n-form-item>
                <n-form-item v-if="form.source !== 'liked'" label="QQ 歌单 ID"
                    ><n-input
                        v-model:value="form.playlistId"
                        :disabled="!!editingId"
                        placeholder="纯数字歌单 ID"
                /></n-form-item>
                <n-form-item
                    v-if="form.source === 'created'"
                    label="个人歌单目录 ID"
                    ><n-input
                        v-model:value="form.dirid"
                        :disabled="!!editingId"
                /></n-form-item>
                <n-form-item label="自动下载音质（必选）"
                    ><n-select
                        v-model:value="form.quality"
                        :options="qualityOptions"
                        placeholder="选择固定音质"
                /></n-form-item>
                <p class="muted">
                    保存在服务端，关闭浏览器后仍然使用；按下载设置允许的顺序降级。
                </p>
                <n-form-item label="检查间隔（分钟）"
                    ><n-input-number
                        v-model:value="form.intervalMinutes"
                        :min="5"
                        :max="10080"
                /></n-form-item>
                <n-form-item label="启用监控"
                    ><n-switch v-model:value="form.enabled"
                /></n-form-item>
                <p class="muted">
                    停用后不再补齐或自动重试；已进入下载队列的任务仍可在任务页管理。
                </p>
            </n-form>
            <n-alert v-if="editorError" type="error">{{ editorError }}</n-alert>
            <template #footer
                ><n-space justify="end"
                    ><n-button @click="showEditor = false">取消</n-button
                    ><n-button type="primary" :loading="busy" @click="save">{{
                        !editingId && form.enabled ? '保存并首次补齐' : '保存'
                    }}</n-button></n-space
                ></template
            >
        </n-modal>

        <n-modal
            v-model:show="showLink"
            preset="card"
            title="关联现有文件"
            class="monitor-modal"
        >
            <n-radio-group v-model:value="linkPath" class="candidate-list">
                <n-radio
                    v-for="file in linking?.candidates"
                    :key="file.path"
                    :value="file.path"
                >
                    <code>{{ file.path }}</code>
                    <p class="muted">
                        标签：{{ file.metadata.title || '缺失' }} ·
                        {{ file.metadata.artists.join('、') || '缺失' }}
                    </p>
                    <p class="muted">
                        文件名：{{ file.filename.title || '未解析' }} ·
                        {{ file.filename.artists.join('、') || '未解析' }}
                    </p>
                    <p v-if="file.warning">{{ file.warning }}</p>
                </n-radio>
            </n-radio-group>
            <n-form-item label="或填写已索引文件的容器内完整路径"
                ><n-input
                    v-model:value="linkPath"
                    placeholder="/music/歌曲.flac"
            /></n-form-item>
            <n-alert v-if="linkError" type="error">{{ linkError }}</n-alert>
            <template #footer
                ><n-button
                    type="primary"
                    :disabled="!linkPath"
                    :loading="busy"
                    @click="confirmLink"
                    >确认关联</n-button
                ></template
            >
        </n-modal>
    </div>
</template>

<script setup lang="ts">
import {
    computed,
    onActivated,
    onDeactivated,
    onUnmounted,
    ref,
    watch,
} from 'vue'
import { useRouter } from 'vue-router'
import {
    NAlert,
    NButton,
    NCard,
    NEmpty,
    NForm,
    NFormItem,
    NInput,
    NInputNumber,
    NModal,
    NPagination,
    NPopconfirm,
    NRadio,
    NRadioGroup,
    NSelect,
    NSpace,
    NSwitch,
    NTag,
} from 'naive-ui'
import * as api from '../api/monitorApi'
import { fetchCreatedPlaylists } from '../api/musicApi'
import { ALL_QUALITY_ORDER, type PlaylistSearchItem } from '../types'
import { useSettingsStore } from '../stores/settingsStore'

const router = useRouter()
const settings = useSettingsStore()
const library = ref<api.LibraryStatus>()
const monitors = ref<api.Monitor[]>([])
const songs = ref<api.MonitorSong[]>([])
const selectedId = ref('')
const selectedName = computed(
    () => monitors.value.find((m) => m.id === selectedId.value)?.name || '歌单',
)
const error = ref(''),
    editorError = ref(''),
    linkError = ref('')
const busy = ref(false),
    scanning = ref(false),
    running = ref(false),
    persistenceFailed = ref(false)
const showEditor = ref(false),
    editingId = ref(''),
    showLink = ref(false)
const linking = ref<api.MonitorSong>(),
    linkPath = ref('')
const filter = ref('all'),
    query = ref(''),
    page = ref(1)
const form = ref<api.MonitorInput>({
    name: '',
    source: 'public',
    playlistId: '',
    dirid: '',
    quality: '',
    intervalMinutes: 60,
    enabled: true,
})
const sourceOptions = [
    { label: 'QQ 公开歌单', value: 'public' },
    { label: '我的 QQ 歌单', value: 'created' },
    { label: '我喜欢', value: 'liked' },
]
const qualityOptions = [...ALL_QUALITY_ORDER]
    .reverse()
    .map((q) => ({ label: q, value: q }))
const filterOptions = [
    { label: '全部状态', value: 'all' },
    { label: '待确认', value: 'pending_confirmation' },
    { label: '等待与下载中', value: 'active' },
    { label: '已处理', value: 'done' },
    { label: '失败', value: 'failed' },
]
const labels: Record<string, string> = {
    pending: '等待匹配',
    ready: '等待入队',
    dispatching: '正在入队',
    queued: '已入队',
    downloading: '下载中',
    paused: '已暂停',
    interrupted: '已中断',
    matched: '已关联',
    downloaded: '已下载',
    ignored: '已忽略',
    pending_confirmation: '待确认',
    no_quality: '无可用音质',
    download_failed: '下载失败',
    credential_invalid: '凭据失效',
}
const stateLabel = (s: string) => labels[s] || s
const sourceLabel = (s: string) =>
    sourceOptions.find((o) => o.value === s)?.label
const canReset = (s: string) =>
    [
        'matched',
        'downloaded',
        'ignored',
        'no_quality',
        'download_failed',
        'credential_invalid',
    ].includes(s)
const stateType = (
    s: string,
): 'default' | 'success' | 'warning' | 'error' | 'info' =>
    ['matched', 'downloaded'].includes(s)
        ? 'success'
        : s === 'pending_confirmation'
          ? 'warning'
          : ['no_quality', 'download_failed', 'credential_invalid'].includes(s)
            ? 'error'
            : 'default'
const time = (seconds: number, fallback = '尚未检查') =>
    seconds ? new Date(seconds * 1000).toLocaleString() : fallback
const filteredSongs = computed(() =>
    songs.value.filter((e) => {
        const category = filter.value
        const matches =
            category === 'all' ||
            e.state === category ||
            (category === 'done' &&
                ['matched', 'downloaded', 'ignored'].includes(e.state)) ||
            (category === 'active' &&
                [
                    'pending',
                    'ready',
                    'dispatching',
                    'queued',
                    'downloading',
                    'paused',
                    'interrupted',
                ].includes(e.state)) ||
            (category === 'failed' &&
                [
                    'no_quality',
                    'download_failed',
                    'credential_invalid',
                ].includes(e.state))
        return (
            matches &&
            `${e.song.title} ${e.song.artist}`
                .toLowerCase()
                .includes(query.value.trim().toLowerCase())
        )
    }),
)
const pageSongs = computed(() =>
    filteredSongs.value.slice((page.value - 1) * 30, page.value * 30),
)
watch([filter, query, selectedId], () => {
    page.value = 1
})
const message = (e: unknown) => (e instanceof Error ? e.message : String(e))
let timer: ReturnType<typeof setTimeout> | undefined
let active = false,
    refreshing = false
let pollGeneration = 0
async function refresh() {
    if (refreshing) return
    refreshing = true
    try {
        const [lib, result] = await Promise.all([
            api.getLibrary(),
            api.getMonitors(),
        ])
        library.value = lib
        monitors.value = result.monitors
        running.value = result.running
        persistenceFailed.value = result.persistenceFailed
        const id = selectedId.value
        if (id) {
            const entries = await api.getMonitorSongs(id)
            if (selectedId.value === id) songs.value = entries
        }
        error.value = ''
    } catch (e) {
        error.value = message(e)
    } finally {
        refreshing = false
    }
}
async function poll(generation: number) {
    await refresh()
    if (active && generation === pollGeneration)
        timer = setTimeout(() => void poll(generation), 5000)
}
function stop() {
    active = false
    pollGeneration++
    clearTimeout(timer)
}
onActivated(() => {
    if (!active) {
        active = true
        void poll(++pollGeneration)
    }
})
onDeactivated(stop)
onUnmounted(stop)
async function selectMonitor(id: string) {
    selectedId.value = id
    songs.value = []
    await refresh()
}
async function perform(action: () => Promise<unknown>) {
    busy.value = true
    try {
        await action()
        await refresh()
    } catch (e) {
        error.value = message(e)
    } finally {
        busy.value = false
    }
}
async function scan() {
    scanning.value = true
    await perform(api.scanLibrary)
    scanning.value = false
}
async function check(m: api.Monitor) {
    await perform(() => api.checkMonitor(m.id))
}
function inputOf(m: api.Monitor): api.MonitorInput {
    return {
        name: m.name,
        source: m.source,
        playlistId: m.playlistId,
        dirid: m.dirid,
        quality: m.quality,
        intervalMinutes: m.intervalMinutes,
        enabled: m.enabled,
    }
}
async function toggle(m: api.Monitor, enabled: boolean) {
    await perform(() => api.saveMonitor({ ...inputOf(m), enabled }, m.id))
}
function edit(m?: api.Monitor) {
    editingId.value = m?.id || ''
    editorError.value = ''
    form.value = m
        ? inputOf(m)
        : {
              name: '',
              source: 'public',
              playlistId: '',
              dirid: '',
              quality:
                  settings.settings.defaultQuality === 'ask'
                      ? ''
                      : settings.settings.defaultQuality,
              intervalMinutes: 60,
              enabled: true,
          }
    showEditor.value = true
}
async function save() {
    if (!form.value.quality || !form.value.intervalMinutes) {
        editorError.value = '请选择自动下载音质并填写检查间隔'
        return
    }
    busy.value = true
    try {
        await api.saveMonitor(form.value, editingId.value || undefined)
        showEditor.value = false
        await refresh()
    } catch (e) {
        editorError.value = message(e)
    } finally {
        busy.value = false
    }
}
async function decide(
    e: api.MonitorSong,
    action: 'download' | 'ignore' | 'reset',
) {
    await perform(() => api.decideSong(e.song.mid, action))
}
function openLink(e: api.MonitorSong) {
    linking.value = e
    linkPath.value = ''
    linkError.value = ''
    showLink.value = true
}
async function confirmLink() {
    if (!linking.value) return
    busy.value = true
    try {
        await api.decideSong(linking.value.song.mid, 'link', linkPath.value)
        showLink.value = false
        await refresh()
    } catch (e) {
        linkError.value = message(e)
    } finally {
        busy.value = false
    }
}
const playlists = ref<PlaylistSearchItem[]>([]),
    loadingPlaylists = ref(false)
const playlistOptions = computed(() =>
    playlists.value.map((p, i) => ({ label: p.name, value: i })),
)
async function loadPlaylists() {
    loadingPlaylists.value = true
    try {
        playlists.value = (await fetchCreatedPlaylists()).playlists
    } catch (e) {
        editorError.value = message(e)
    } finally {
        loadingPlaylists.value = false
    }
}
function choosePlaylist(i: number) {
    const p = playlists.value[i]
    if (p) {
        form.value.playlistId = p.id
        form.value.dirid = p.dirid || ''
        form.value.name = p.name
    }
}
</script>

<style scoped>
.monitor-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1100px;
    margin: 0 auto;
}
.toolbar,
.song-header {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
}
.toolbar h2 {
    flex: 1;
    margin: 0;
}
.muted {
    color: var(--color-text-secondary);
    font-size: 13px;
}
.root-row,
.candidate-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 12px 0;
}
.actions {
    margin-top: 12px;
}
.song-filters {
    display: flex;
    gap: 12px;
}
.song-filters > * {
    flex: 1;
    min-width: 0;
}
.song-row {
    padding: 16px 0;
    border-top: 1px solid var(--border-color);
}
.song-row p {
    margin: 6px 0;
}
code,
p,
strong {
    overflow-wrap: anywhere;
}
.monitor-modal {
    width: min(640px, 94vw);
    max-height: 90vh;
    overflow: auto;
}
@media (max-width: 767px) {
    .song-filters {
        flex-direction: column;
    }
    .actions :deep(button) {
        min-height: 44px;
    }
}
</style>

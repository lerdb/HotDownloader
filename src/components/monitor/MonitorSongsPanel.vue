<template>
    <n-card
        v-if="selectedId"
        :title="`${selectedName} · 逐曲状态`"
        size="small"
    >
        <div class="song-filters">
            <n-select v-model:value="filter" :options="filterOptions" /><n-input
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
        <n-space class="batch-actions" align="center">
            <n-checkbox
                :checked="allPagePendingSelected"
                :indeterminate="
                    somePagePendingSelected && !allPagePendingSelected
                "
                :disabled="busy || !pagePending.length"
                @update:checked="emit('select-page', $event)"
            >
                选择本页待确认
            </n-checkbox>
            <span>已选 {{ selectedMids.length }} 首（最多 200 首）</span>
            <n-button
                :disabled="busy || !selectedMids.length"
                @click="selectedMids = []"
                >清空选择</n-button
            >
            <n-popconfirm
                :show-icon="false"
                @positive-click="emit('batch-decide', 'download')"
            >
                <template #trigger
                    ><n-button :disabled="busy || !selectedMids.length"
                        >下载所选</n-button
                    ></template
                >
                将下载所选
                {{ selectedMids.length }}
                首待确认歌曲，使用当前监控音质；决定对其他歌单中的相同歌曲也生效，已有文件保留。
            </n-popconfirm>
            <n-popconfirm
                :show-icon="false"
                @positive-click="emit('batch-decide', 'ignore')"
            >
                <template #trigger
                    ><n-button :disabled="busy || !selectedMids.length"
                        >忽略所选</n-button
                    ></template
                >
                将忽略所选
                {{ selectedMids.length }}
                首待确认歌曲，对其他歌单中的相同歌曲也生效。
            </n-popconfirm>
        </n-space>
        <n-alert v-if="batchMessage" type="success">{{ batchMessage }}</n-alert>
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
                <n-checkbox
                    v-if="entry.state === 'pending_confirmation'"
                    :checked="selectedMids.includes(entry.song.mid)"
                    :disabled="
                        busy ||
                        (selectedMids.length >= 200 &&
                            !selectedMids.includes(entry.song.mid))
                    "
                    :aria-label="`选择 ${entry.song.title}`"
                    @update:checked="
                        emit('select-song', entry.song.mid, $event)
                    "
                />
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
                    <n-button :disabled="busy" @click="emit('link', entry)"
                        >关联现有文件</n-button
                    >
                    <n-button
                        :disabled="busy"
                        @click="emit('decide', entry, 'download')"
                        >下载</n-button
                    >
                    <n-button
                        :disabled="busy"
                        @click="emit('decide', entry, 'ignore')"
                        >忽略</n-button
                    >
                </template>
                <n-button
                    v-if="entry.taskId"
                    @click="
                        router.push({
                            path: '/task',
                            query: { taskId: entry.taskId },
                        })
                    "
                    >查看任务</n-button
                >
                <n-button
                    v-if="entry.state === 'credential_invalid'"
                    @click="router.push('/settings')"
                    >前往登录</n-button
                >
                <n-popconfirm
                    v-if="canReset(entry.state)"
                    @positive-click="emit('decide', entry, 'reset')"
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
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import {
    NAlert,
    NButton,
    NCard,
    NCheckbox,
    NEmpty,
    NInput,
    NPagination,
    NPopconfirm,
    NSelect,
    NSpace,
    NTag,
} from 'naive-ui'
import type { MonitorSong } from '../../api/monitorApi'
import {
    time,
    stateLabel,
    stateType,
    canReset,
    filterOptions,
} from './presentation'
const router = useRouter()
const filter = defineModel<string>('filter', { required: true })
const query = defineModel<string>('query', { required: true })
const page = defineModel<number>('page', { required: true })
const selectedMids = defineModel<string[]>('selectedMids', { required: true })
defineProps<{
    selectedId: string
    selectedName: string
    busy: boolean
    batchMessage: string
    filteredSongs: MonitorSong[]
    pageSongs: MonitorSong[]
    pagePending: MonitorSong[]
    allPagePendingSelected: boolean
    somePagePendingSelected: boolean
}>()
const emit = defineEmits<{
    'select-page': [checked: boolean]
    'select-song': [mid: string, checked: boolean]
    'batch-decide': [action: 'download' | 'ignore']
    link: [entry: MonitorSong]
    decide: [entry: MonitorSong, action: 'download' | 'ignore' | 'reset']
}>()
</script>

<style scoped src="./monitor-shared.css"></style>
<style scoped>
.song-header {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
}
.song-filters {
    display: flex;
    gap: 12px;
}
.song-filters > * {
    flex: 1;
    min-width: 0;
}
@media (max-width: 767px) {
    .song-filters {
        flex-direction: column;
    }
}
</style>

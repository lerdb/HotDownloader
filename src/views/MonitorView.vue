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
        <LibraryStatusCard
            :library="library"
            :scanning="scanning"
            :busy="busy"
            @scan="scan"
        />

        <p class="muted">
            首次补齐当前缺失歌曲，之后只处理新增歌曲。处理决定按 MID
            跨歌单共享。{{ running ? '服务正在检查或补齐…' : '' }}
        </p>
        <n-empty
            v-if="!monitors.length"
            description="添加 QQ 歌单或“我喜欢”，开始自动补齐"
        />
        <MonitorCard
            v-for="monitor in monitors"
            :key="monitor.id"
            :monitor="monitor"
            :selected="selectedId === monitor.id"
            :busy="busy"
            @toggle="toggle(monitor, $event)"
            @check="check(monitor)"
            @edit="edit(monitor)"
            @select="selectMonitor(monitor.id)"
            @remove="remove(monitor)"
            @history="openHistory(monitor)"
        />

        <MonitorSongsPanel
            :selected-id="selectedId"
            :selected-name="selectedName"
            :busy="busy"
            :batch-message="batchMessage"
            :filtered-songs="filteredSongs"
            :page-songs="pageSongs"
            :page-pending="pagePending"
            :all-page-pending-selected="allPagePendingSelected"
            :some-page-pending-selected="somePagePendingSelected"
            v-model:filter="filter"
            v-model:query="query"
            v-model:page="page"
            v-model:selected-mids="selectedMids"
            @select-page="selectPagePending"
            @select-song="selectSong"
            @batch-decide="batchDecide"
            @decide="decide"
            @link="openLink"
        />

        <MonitorHistoryModal
            v-model:show-history="showHistory"
            v-model:history-page="historyPage"
            :history-name="historyName"
            :history-error="historyError"
            :history-loading="historyLoading"
            :history="history"
            @refresh="loadHistory"
        />

        <MonitorEditorModal
            v-model:show-editor="showEditor"
            v-model:form="form"
            :editing-id="editingId"
            :editor-error="editorError"
            :busy="busy"
            :duplicate-monitor="duplicateMonitor"
            :loading-playlists="loadingPlaylists"
            :playlist-options="playlistOptions"
            @load-playlists="loadPlaylists"
            @choose-playlist="choosePlaylist"
            @edit="edit"
            @save="save"
        />

        <MonitorLinkModal
            v-model:show-link="showLink"
            v-model:link-path="linkPath"
            :linking="linking"
            :link-error="linkError"
            :busy="busy"
            @confirm="confirmLink"
        />
    </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { NAlert, NButton, NEmpty } from 'naive-ui'
import { useMonitorPage } from '../composables/useMonitorPage'
import LibraryStatusCard from '../components/monitor/LibraryStatusCard.vue'
import MonitorCard from '../components/monitor/MonitorCard.vue'
import MonitorSongsPanel from '../components/monitor/MonitorSongsPanel.vue'
import MonitorHistoryModal from '../components/monitor/MonitorHistoryModal.vue'
import MonitorEditorModal from '../components/monitor/MonitorEditorModal.vue'
import MonitorLinkModal from '../components/monitor/MonitorLinkModal.vue'
const router = useRouter()
const {
    library,
    monitors,
    selectedMids,
    batchMessage,
    showHistory,
    historyLoading,
    historyError,
    historyName,
    history,
    historyPage,
    openHistory,
    loadHistory,
    selectedId,
    selectedName,
    error,
    editorError,
    linkError,
    busy,
    scanning,
    running,
    persistenceFailed,
    showEditor,
    editingId,
    showLink,
    linking,
    linkPath,
    filter,
    query,
    page,
    form,
    duplicateMonitor,
    filteredSongs,
    pageSongs,
    pagePending,
    allPagePendingSelected,
    somePagePendingSelected,
    selectSong,
    selectPagePending,
    selectMonitor,
    scan,
    check,
    remove,
    batchDecide,
    toggle,
    edit,
    save,
    decide,
    openLink,
    confirmLink,
    loadingPlaylists,
    playlistOptions,
    loadPlaylists,
    choosePlaylist,
} = useMonitorPage()
</script>

<style scoped>
.monitor-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1100px;
    margin: 0 auto;
}
.toolbar {
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
p {
    overflow-wrap: anywhere;
}
</style>

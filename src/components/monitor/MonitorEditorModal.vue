<template>
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
                        @click="emit('load-playlists')"
                        >读取我的歌单</n-button
                    ><n-select
                        :options="playlistOptions"
                        placeholder="选择个人歌单"
                        @update:value="emit('choose-playlist', $event)"
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
                ><n-input v-model:value="form.dirid" :disabled="!!editingId"
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
        <n-alert
            v-if="duplicateMonitor && !editingId"
            type="warning"
            title="此歌单已有监控"
        >
            “{{ duplicateMonitor.name }}”{{
                duplicateMonitor.enabled ? '已启用' : '已停用'
            }}。
            <n-button :disabled="busy" @click="emit('edit', duplicateMonitor)"
                >编辑已有监控</n-button
            >
        </n-alert>
        <template #footer
            ><n-space justify="end"
                ><n-button @click="showEditor = false">取消</n-button
                ><n-button
                    type="primary"
                    :loading="busy"
                    :disabled="!editingId && !!duplicateMonitor"
                    @click="emit('save')"
                    >{{
                        !editingId && form.enabled ? '保存并首次补齐' : '保存'
                    }}</n-button
                ></n-space
            ></template
        >
    </n-modal>
</template>

<script setup lang="ts">
import {
    NAlert,
    NButton,
    NForm,
    NFormItem,
    NInput,
    NInputNumber,
    NModal,
    NSelect,
    NSpace,
    NSwitch,
} from 'naive-ui'
import type { Monitor, MonitorInput } from '../../api/monitorApi'
import { sourceOptions } from './presentation'
import { ALL_QUALITY_ORDER } from '../../types'
const showEditor = defineModel<boolean>('showEditor', { required: true })
const form = defineModel<MonitorInput>('form', { required: true })
const qualityOptions = [...ALL_QUALITY_ORDER]
    .reverse()
    .map((q) => ({ label: q, value: q }))
defineProps<{
    editingId: string
    editorError: string
    busy: boolean
    duplicateMonitor?: Monitor
    loadingPlaylists: boolean
    playlistOptions: { label: string; value: number }[]
}>()
const emit = defineEmits<{
    'load-playlists': []
    'choose-playlist': [index: number]
    edit: [monitor: Monitor]
    save: []
}>()
</script>

<style scoped src="./monitor-shared.css"></style>

<template>
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
            ><n-input v-model:value="linkPath" placeholder="/music/歌曲.flac"
        /></n-form-item>
        <n-alert v-if="linkError" type="error">{{ linkError }}</n-alert>
        <template #footer
            ><n-button
                type="primary"
                :disabled="!linkPath"
                :loading="busy"
                @click="emit('confirm')"
                >确认关联</n-button
            ></template
        >
    </n-modal>
</template>

<script setup lang="ts">
import {
    NAlert,
    NButton,
    NFormItem,
    NInput,
    NModal,
    NRadio,
    NRadioGroup,
} from 'naive-ui'
import type { MonitorSong } from '../../api/monitorApi'
const showLink = defineModel<boolean>('showLink', { required: true })
const linkPath = defineModel<string>('linkPath', { required: true })
defineProps<{ linking?: MonitorSong; linkError: string; busy: boolean }>()
const emit = defineEmits<{ confirm: [] }>()
</script>

<style scoped src="./monitor-shared.css"></style>
<style scoped>
.candidate-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 12px 0;
}
</style>

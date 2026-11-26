<template>
    <template v-if="isNarrow">
        <!-- 移动端：开关行内布局，与其他开关组件保持一致 -->
        <div class="setting-row">
            <span class="setting-label">下载时同步保存歌词文件</span>
            <n-switch
                :value="settingsStore.settings.downloadLrc"
                @update:value="
                    (val) => (settingsStore.settings.downloadLrc = val)
                "
            />
        </div>
    </template>
    <template v-else>
        <!-- 桌面端：原有表单布局 -->
        <n-form-item label="下载时同步保存歌词文件">
            <n-switch
                :value="settingsStore.settings.downloadLrc"
                @update:value="
                    (val) => (settingsStore.settings.downloadLrc = val)
                "
            />
        </n-form-item>
    </template>
    <template v-if="settingsStore.settings.downloadLrc">
        <n-form-item label="歌词文件格式">
            <n-select
                v-model:value="settingsStore.settings.downloadLrcFormat"
                :options="formats"
            />
        </n-form-item>
        <div v-if="isNarrow" class="setting-row">
            <span class="setting-label">歌词文件包含翻译</span>
            <n-switch
                v-model:value="settingsStore.settings.downloadLrcTranslation"
            />
        </div>
        <n-form-item v-else label="歌词文件包含翻译">
            <n-switch
                v-model:value="settingsStore.settings.downloadLrcTranslation"
            />
        </n-form-item>
    </template>
    <template
        v-if="
            settingsStore.settings.downloadLrc ||
            settingsStore.settings.writeMetadata
        "
    >
        <div v-if="isNarrow" class="setting-row">
            <span class="setting-label">歌词文件和内嵌歌词包含罗马音</span>
            <n-switch
                v-model:value="settingsStore.settings.lyricRomanization"
            />
        </div>
        <n-form-item v-else label="附加罗马音">
            <n-switch
                v-model:value="settingsStore.settings.lyricRomanization"
            />
            <span class="setting-description">用于歌词文件和内嵌歌词</span>
        </n-form-item>
    </template>
</template>

<script setup lang="ts">
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { NFormItem, NSelect, NSwitch } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'

const settingsStore = useSettingsStore()
const formats = [
    { label: '普通 LRC（逐行）', value: 'lrc' },
    { label: 'ELRC（逐字，保存为 .lrc）', value: 'elrc' },
]

// 移动端判断
const isNarrow = useNarrowLayout()
</script>

<style scoped>
.setting-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
}

.setting-label {
    font-size: 14px;
    color: var(--n-text-color);
}

.setting-description {
    margin-inline-start: 12px;
    color: var(--color-text-secondary);
    font-size: 12px;
}
</style>

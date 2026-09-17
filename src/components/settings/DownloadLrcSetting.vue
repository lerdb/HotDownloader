<template>
    <template v-if="isNarrow">
        <!-- 移动端：开关行内布局，与其他开关组件保持一致 -->
        <div class="setting-row">
            <span class="setting-label">下载时同步保存 LRC 歌词文件</span>
            <n-switch :value="settingsStore.settings.downloadLrc"
                @update:value="(val) => (settingsStore.settings.downloadLrc = val)" />
        </div>
    </template>
    <template v-else>
        <!-- 桌面端：原有表单布局 -->
        <n-form-item label="下载时同步保存 LRC 歌词文件">
            <n-switch :value="settingsStore.settings.downloadLrc"
                @update:value="(val) => (settingsStore.settings.downloadLrc = val)" />
        </n-form-item>
    </template>
</template>

<script setup lang="ts">
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { NFormItem, NSwitch } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'

const settingsStore = useSettingsStore()

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
</style>
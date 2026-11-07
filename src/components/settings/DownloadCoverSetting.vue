<template>
    <div v-if="isNarrow" class="setting-row">
        <span class="setting-label">下载时同步保存封面文件</span>
        <n-switch v-model:value="settingsStore.settings.downloadCover" />
    </div>
    <n-form-item v-else label="下载时同步保存封面文件">
        <n-switch v-model:value="settingsStore.settings.downloadCover" />
    </n-form-item>
</template>

<script setup lang="ts">
import { NFormItem, NSwitch } from 'naive-ui'
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { useSettingsStore } from '../../stores/settingsStore'

const settingsStore = useSettingsStore()
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

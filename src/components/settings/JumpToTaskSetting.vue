<template>
    <template v-if="isNarrow">
        <div class="setting-row">
            <span class="setting-label">添加任务后跳转至任务页</span>
            <n-switch :value="settingsStore.settings.jumpToTask"
                @update:value="(val) => (settingsStore.settings.jumpToTask = val)" />
        </div>
    </template>
    <template v-else>
        <n-form-item label="添加任务后跳转至任务页">
            <n-switch :value="settingsStore.settings.jumpToTask"
                @update:value="(val) => (settingsStore.settings.jumpToTask = val)" />
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
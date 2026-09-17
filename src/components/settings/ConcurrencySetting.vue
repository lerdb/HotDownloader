<template>
    <template v-if="isNarrow">
        <div class="setting-row">
            <span class="setting-label">同时下载数</span>
            <n-input-number :value="settingsStore.settings.maxConcurrent"
                @update:value="(val) => (settingsStore.settings.maxConcurrent = val ?? 1)" :min="1" :max="10" step="1"
                button-placement="both" class="concurrency-input" />
        </div>
    </template>
    <template v-else>
        <n-form-item label="同时下载数">
            <n-input-number :value="settingsStore.settings.maxConcurrent"
                @update:value="(val) => (settingsStore.settings.maxConcurrent = val ?? 1)" :min="1" :max="10" step="1"
                button-placement="both" class="concurrency-input" />
        </n-form-item>
    </template>
</template>

<script setup lang="ts">
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { NFormItem, NInputNumber } from 'naive-ui'
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

.concurrency-input :deep(.n-input__input-el) {
    text-align: center;
}

.concurrency-input :deep(.n-input__input) {
    display: flex;
    align-items: center;
}
</style>

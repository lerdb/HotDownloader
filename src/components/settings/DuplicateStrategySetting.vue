<template>
    <template v-if="isNarrow">
        <div class="setting-row">
            <span class="setting-label">重名文件处理策略</span>
            <n-radio-group :value="settingsStore.settings.duplicateStrategy"
                @update:value="(val) => (settingsStore.settings.duplicateStrategy = val)" size="small">
                <n-radio-button value="ask">询问</n-radio-button>
                <n-radio-button value="overwrite">覆盖</n-radio-button>
                <n-radio-button value="rename">保留两份</n-radio-button>
                <n-radio-button value="cancel">取消</n-radio-button>
            </n-radio-group>
        </div>
    </template>
    <template v-else>
        <n-form-item label="重名文件处理策略">
            <n-radio-group :value="settingsStore.settings.duplicateStrategy"
                @update:value="(val) => (settingsStore.settings.duplicateStrategy = val)">
                <n-radio-button value="ask">询问</n-radio-button>
                <n-radio-button value="overwrite">覆盖</n-radio-button>
                <n-radio-button value="rename">保留两份</n-radio-button>
                <n-radio-button value="cancel">取消</n-radio-button>
            </n-radio-group>
        </n-form-item>
    </template>
</template>

<script setup lang="ts">
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { NFormItem, NRadioGroup, NRadioButton } from 'naive-ui'
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
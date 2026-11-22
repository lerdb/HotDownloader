<template>
    <div v-if="selectedCount > 0" class="batch-actions">
        <span class="batch-label">
            <span class="batch-count">{{ selectedCount }}</span>
            个任务已选择
        </span>
        <div class="batch-btns">
            <n-button size="small" round @click="emit('pause')"
                >暂停所选</n-button
            >
            <n-button size="small" round @click="emit('resume')"
                >恢复所选</n-button
            >
            <n-popconfirm
                :style="{ maxWidth: 'calc(100vw - 32px)' }"
                @positive-click="handleConfirm"
            >
                <template #trigger>
                    <n-button type="error" size="small" round>清除所选</n-button>
                </template>
                <n-space vertical :size="8">
                    <span>确认清除所选任务吗？</span>
                    <n-checkbox v-model:checked="deleteFile">
                        同时删除已下载或未完成的文件
                    </n-checkbox>
                </n-space>
            </n-popconfirm>
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NButton, NPopconfirm, NCheckbox, NSpace } from 'naive-ui'

defineProps<{
    selectedCount: number
}>()

const emit = defineEmits<{
    (e: 'pause'): void
    (e: 'resume'): void
    (e: 'clear', deleteFile: boolean): void
}>()

const deleteFile = ref(false)

function handleConfirm() {
    emit('clear', deleteFile.value)
    deleteFile.value = false
}
</script>

<style scoped>
.batch-actions {
    position: sticky;
    bottom: 8px;
    z-index: 2;
    display: flex;
    flex-wrap: wrap;
    flex-shrink: 0;
    gap: 10px 16px;
    align-items: center;
    justify-content: space-between;
    padding: 12px 18px;
    background: var(--bg-glass-strong);
    backdrop-filter: blur(20px) saturate(160%);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--border-color-strong);
    border-radius: 16px;
    box-shadow: var(--shadow-md);
    margin-top: 14px;
}

.batch-label {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 500;
}

.batch-count {
    min-width: 24px;
    height: 24px;
    padding: 0 8px;
    border-radius: 999px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-size: 13px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-variant-numeric: tabular-nums;
}

.batch-btns {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
}

@media (max-width: 767px) {
    .batch-actions {
        padding: 10px 14px;
    }

    .batch-actions :deep(.n-button) {
        min-height: 40px;
    }
}
</style>

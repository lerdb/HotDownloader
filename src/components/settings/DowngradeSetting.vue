<template>
    <template v-if="isNarrow">
        <!-- 移动端：开关保持行内布局，排序入口单独占一行，避免窄屏拥挤。 -->
        <div class="setting-row">
            <span class="setting-label">自动降级</span>
            <n-switch :value="settingsStore.settings.autoDowngrade"
                @update:value="(val) => (settingsStore.settings.autoDowngrade = val)" />
        </div>
    </template>
    <template v-else>
        <n-form-item label="自动降级">
            <n-switch :value="settingsStore.settings.autoDowngrade"
                @update:value="(val) => (settingsStore.settings.autoDowngrade = val)" />
        </n-form-item>
    </template>

    <n-form-item label="降级顺序">
        <div class="downgrade-setting">
            <div class="downgrade-summary" :title="downgradeOrderText" :aria-label="`当前降级顺序：${downgradeOrderText}`">
                {{ downgradeOrderText }}
            </div>
            <p class="downgrade-help">
                <template v-if="settingsStore.settings.autoDowngrade">
                    目标音质不可用时，将从它的下一项开始，按从上到下的顺序依次尝试。
                </template>
                <template v-else>
                    当前顺序已保留，开启自动降级后可编辑并生效。
                </template>
            </p>
            <n-button size="small" :disabled="!settingsStore.settings.autoDowngrade" @click="openEditor">
                自定义顺序
            </n-button>
        </div>
    </n-form-item>

    <!--
        排序过程只修改 draftOrder，点击“保存”后才一次性写回 Pinia。
        这样“取消”能完整撤销本次编辑，也不会让每次上移/下移都触发设置持久化。
    -->
    <n-modal v-model:show="showEditor" preset="card" title="自定义降级顺序" class="downgrade-modal"
        style="width: min(480px, calc(100vw - 32px));" :mask-closable="false">
        <p id="downgrade-order-help" class="editor-help">
            排在目标音质之后的项目才会作为降级候选。目标音质本身可用时仍会直接下载。
        </p>

        <ol class="quality-order-list" aria-describedby="downgrade-order-help">
            <li v-for="(quality, index) in draftOrder" :key="quality" class="quality-order-item">
                <span class="quality-index" aria-hidden="true">{{ index + 1 }}</span>
                <span class="quality-name">{{ quality }}</span>
                <n-tag v-if="quality === settingsStore.settings.defaultQuality" size="small" :bordered="false">
                    默认
                </n-tag>
                <div class="move-actions">
                    <n-button size="small" quaternary :disabled="index === 0"
                        :aria-label="`上移 ${quality}`" @click="moveQuality(index, -1)">
                        ↑
                    </n-button>
                    <n-button size="small" quaternary :disabled="index === draftOrder.length - 1"
                        :aria-label="`下移 ${quality}`" @click="moveQuality(index, 1)">
                        ↓
                    </n-button>
                </div>
            </li>
        </ol>

        <!-- 视觉隐藏的实时区域会向读屏软件报告排序结果。 -->
        <span class="sr-only" aria-live="polite">{{ liveMessage }}</span>

        <template #footer>
            <div class="modal-actions">
                <n-button size="small" text :disabled="isDefaultOrder" @click="resetDraftOrder">
                    恢复默认顺序
                </n-button>
                <div class="modal-primary-actions">
                    <n-button @click="cancelEditor">取消</n-button>
                    <n-button type="primary" :disabled="!hasChanges" @click="saveOrder">保存</n-button>
                </div>
            </div>
        </template>
    </n-modal>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { NButton, NFormItem, NModal, NSwitch, NTag } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { QUALITY_DOWNGRADE_ORDER } from '../../types'

const settingsStore = useSettingsStore()

const showEditor = ref(false)
const draftOrder = ref<string[]>([])
const liveMessage = ref('')

const downgradeOrderText = computed(() =>
    settingsStore.settings.qualityDowngradeOrder.join(' → ')
)

const hasChanges = computed(() =>
    draftOrder.value.join('\u0000') !== settingsStore.settings.qualityDowngradeOrder.join('\u0000')
)

const isDefaultOrder = computed(() =>
    draftOrder.value.join('\u0000') === QUALITY_DOWNGRADE_ORDER.join('\u0000')
)

/** 打开编辑器时克隆已保存顺序，保证关闭或取消不会污染持久设置。 */
function openEditor() {
    draftOrder.value = [...settingsStore.settings.qualityDowngradeOrder]
    liveMessage.value = ''
    showEditor.value = true
}

function cancelEditor() {
    showEditor.value = false
}

/**
 * 将指定音质移动一格。这里不直接修改设置：用户可以连续调整并预览，最后只在
 * 点击“保存”时提交一次。上/下移按钮同时兼容鼠标、键盘和触屏操作。
 */
function moveQuality(index: number, offset: -1 | 1) {
    const targetIndex = index + offset
    if (targetIndex < 0 || targetIndex >= draftOrder.value.length) return

    const next = [...draftOrder.value]
    const [quality] = next.splice(index, 1)
    next.splice(targetIndex, 0, quality)
    draftOrder.value = next
    liveMessage.value = `已将 ${quality} 移至第 ${targetIndex + 1} 位`
}

/** 恢复只作用于弹窗草稿；用户仍可通过“取消”保留之前保存的自定义顺序。 */
function resetDraftOrder() {
    draftOrder.value = [...QUALITY_DOWNGRADE_ORDER]
    liveMessage.value = '已恢复默认顺序，点击保存后生效'
}

function saveOrder() {
    // 使用新数组写回，避免后续编辑草稿时与 store 共用同一个数组引用。
    settingsStore.settings.qualityDowngradeOrder = [...draftOrder.value]
    showEditor.value = false
}

// 移动端判断
const isNarrow = ref(
    typeof window !== 'undefined' &&
    window.matchMedia('(max-width: 767px)').matches
)
let mediaQuery: MediaQueryList | null = null

function updateNarrow(e: MediaQueryListEvent | MediaQueryList) {
    isNarrow.value = e.matches
}

onMounted(() => {
    mediaQuery = window.matchMedia('(max-width: 767px)')
    updateNarrow(mediaQuery)
    mediaQuery.addEventListener('change', updateNarrow)
})

onUnmounted(() => {
    if (mediaQuery) {
        mediaQuery.removeEventListener('change', updateNarrow)
    }
})
</script>

<style scoped>
.downgrade-setting {
    min-width: 0;
    width: 100%;
}

.downgrade-summary {
    max-width: 100%;
    overflow: hidden;
    color: var(--color-text);
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.downgrade-help,
.editor-help {
    color: var(--color-text-secondary);
    font-size: 12px;
    line-height: 1.5;
}

.downgrade-help {
    margin: 4px 0 8px;
}

.editor-help {
    margin-bottom: 12px;
}

.quality-order-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 58vh;
    overflow-y: auto;
    padding-right: 4px;
}

.quality-order-item {
    display: flex;
    min-height: 38px;
    align-items: center;
    gap: 10px;
    padding: 4px 6px 4px 10px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
}

.quality-index {
    width: 22px;
    flex: 0 0 22px;
    color: var(--color-text-secondary);
    font-size: 12px;
    text-align: right;
}

.quality-name {
    min-width: 0;
    flex: 1;
    color: var(--color-text);
    overflow-wrap: anywhere;
}

.move-actions,
.modal-primary-actions,
.modal-actions {
    display: flex;
    align-items: center;
}

.move-actions {
    gap: 2px;
}

.modal-primary-actions {
    gap: 8px;
}

.modal-actions {
    justify-content: space-between;
    gap: 16px;
}

/* 移动端行内布局 */
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

.sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
}

@media (max-width: 767px) {
    .quality-order-item {
        min-height: 48px;
    }

    .move-actions :deep(.n-button) {
        min-width: 44px;
        min-height: 44px;
    }

    .modal-actions {
        align-items: stretch;
        flex-direction: column;
    }

    .modal-primary-actions {
        justify-content: flex-end;
    }
}
</style>
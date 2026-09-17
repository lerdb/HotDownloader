<template>
    <div class="search-bar">
        <!-- 平台选择下拉 -->
        <n-dropdown :options="platformDropdownOptions" trigger="click" @select="handlePlatformSelect">
            <n-button quaternary size="small" class="platform-btn">
                <span class="platform-label">{{ currentPlatformLabel }}</span>
                <span class="platform-arrow">▾</span>
            </n-button>
        </n-dropdown>

        <n-input v-model:value="keywordModel" :placeholder="placeholder" clearable @keyup.enter="handleSearch"
            @clear="handleClear" class="search-input" />
        <n-button type="primary" @click="handleSearch" :disabled="!keywordModel.trim() || loading" :loading="loading"
            class="search-btn">
            {{ buttonText }}
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { NInput, NButton, NDropdown } from 'naive-ui'
import type { PlatformOption } from '../../config/platforms'

const props = withDefaults(
    defineProps<{
        keyword: string
        placeholder?: string
        buttonText?: string
        loading?: boolean
        platform: string
        platformOptions: PlatformOption[]
    }>(),
    {
        placeholder: '搜索歌曲、歌手、专辑',
        buttonText: '搜索',
        loading: false,
    }
)

const emit = defineEmits<{
    (e: 'update:keyword', value: string): void
    (e: 'update:platform', value: string): void
    (e: 'search'): void
    (e: 'clear'): void
}>()

const keywordModel = ref(props.keyword)

// 当前平台对应的显示 label
const currentPlatformLabel = computed(() => {
    const found = props.platformOptions.find(p => p.key === props.platform)
    return found ? found.label : props.platform
})

// 下拉选项格式：Naive UI 需要 { label, key } 结构
const platformDropdownOptions = computed(() => {
    return props.platformOptions.map(p => ({
        label: p.label,
        key: p.key,
    }))
})

// 平台选择处理：触发 update:platform 事件，父组件更新 platform 值
function handlePlatformSelect(key: string) {
    emit('update:platform', key)
}

// 向上同步 keyword
watch(keywordModel, (val) => {
    emit('update:keyword', val)
})

// 向下同步 keyword：当父组件 keyword 变化时更新输入框
watch(
    () => props.keyword,
    (newVal) => {
        if (newVal !== keywordModel.value) {
            keywordModel.value = newVal
        }
    }
)

function handleSearch() {
    if (keywordModel.value.trim()) {
        emit('search')
    }
}

function handleClear() {
    // 点击清空按钮时，输入框已经变为空，同时通知父组件清理页面状态
    emit('clear')
}
</script>

<!-- ===== 必要的 CSS 变量（仅布局/主题，非组件颜色） ===== -->
<style scoped>
.search-bar {
    --search-height: 44px;
    --search-font-size: 14px;
    --search-radius: 8px;
    /* 统一圆角 */
    --search-padding: 8px;
    --search-gap: 8px;

    /* 容器背景/阴影（支持深色模式） */
    --search-bg: var(--bg-sidebar);
    --search-shadow: 0 1px 3px rgba(0, 0, 0, 0.03);
    --search-shadow-focus: 0 2px 8px rgba(0, 0, 0, 0.06);
}
</style>

<!-- ===== 组件样式（仅布局覆盖，颜色/圆角尽量用原生） ===== -->
<style scoped>
.search-bar {
    display: flex;
    align-items: center;
    gap: var(--search-gap);
    min-width: 0;
    margin-bottom: 16px;
    background: var(--search-bg);
    padding: var(--search-padding);
    border: 1px solid var(--border-color);
    border-radius: var(--search-radius);
    /* 外层圆角 */
    box-shadow: var(--search-shadow);
    transition: box-shadow 0.2s ease;
}

.search-bar:focus-within {
    box-shadow: var(--search-shadow-focus);
}

/* 平台按钮样式 */
.platform-btn {
    height: var(--search-height) !important;
    min-width: 52px;
    padding: 0 8px !important;
    font-size: 13px !important;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
}

.platform-arrow {
    margin-left: 4px;
    color: var(--color-text-secondary);
}

/* 输入框容器自动撑开 */
.search-bar .search-input {
    flex: 1;
    min-width: 0;
}

/* ---------- 让 n-input 透明，只继承外层背景 ---------- */
.search-bar .search-input {
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
    height: var(--search-height) !important;
    --n-padding-left: 12px;
    --n-padding-right: 12px;
    /* 左右缩进，占位符不再顶左 */
    border-radius: 0 !important;
    /* 取消自身圆角，由外层统一 */
}

.search-input :deep(.n-input-wrapper) {
    background: transparent !important;
    border: none !important;
    height: 100% !important;
}

/* 内部 input 零内边距，由父级控制 */
.search-input :deep(.n-input__input) {
    padding: 0 !important;
    font-size: var(--search-font-size) !important;
    height: 100% !important;
    line-height: var(--search-height) !important;
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
    color: inherit !important;
    /* 使用 Naive UI 默认文字颜色 */
}

.search-input :deep(.n-input__input-el) {
    height: 100%;
}

.search-input :deep(.n-input__input-el::placeholder) {
    color: inherit !important;
    /* 使用 Naive UI 默认占位符颜色 */
    opacity: 0.6;
}

/* 隐藏内置边框伪元素 */
.search-input :deep(.n-input__border),
.search-input :deep(.n-input__state-border) {
    display: none !important;
}

/* 清除按钮与输入文字居中对齐，沿用组件的清除交互 */
.search-input :deep(.n-input__suffix) {
    align-items: center;
}

/* ---------- 按钮：仅控制尺寸，颜色/圆角完全由 type="primary" 决定 ---------- */
.search-bar .search-btn {
    height: var(--search-height) !important;
    padding: 0 18px !important;
    font-size: var(--search-font-size) !important;
    border-radius: var(--search-radius) !important;
    /* 与外层圆角一致 */
    display: inline-flex !important;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
}

/* 保留悬停/禁用等状态，利用 Naive UI 的默认行为，仅微调阴影 */
.search-bar .search-btn:hover:not(:disabled) {
    box-shadow: var(--search-shadow-focus);
}

/* 窄屏不再将操作按钮单独放一行 */
/*
@media (max-width: 767px) {
    .search-bar {
        display: grid;
        grid-template-columns: auto minmax(0, 1fr);
    }

    .search-bar .search-btn {
        grid-column: 1 / -1;
    }
}
*/
</style>

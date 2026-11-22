<template>
    <div class="search-bar">
        <!-- 平台选择下拉 -->
        <n-dropdown
            :options="platformDropdownOptions"
            trigger="click"
            @select="handlePlatformSelect"
        >
            <button type="button" class="platform-pill" aria-label="选择音乐平台">
                <span
                    class="platform-dot"
                    :style="{ backgroundColor: platformDotColor }"
                    aria-hidden="true"
                />
                <span class="platform-label">{{ currentPlatformLabel }}</span>
                <span class="platform-arrow" aria-hidden="true">▾</span>
            </button>
        </n-dropdown>

        <span class="search-leading-icon" aria-hidden="true">
            <AppIcon name="search" :size="17" />
        </span>

        <n-input
            v-model:value="keywordModel"
            :placeholder="placeholder"
            clearable
            @keyup.enter="handleSearch"
            @clear="handleClear"
            class="search-input"
        />

        <n-button
            type="primary"
            @click="handleSearch"
            :disabled="!keywordModel.trim() || loading"
            :loading="loading"
            class="search-btn"
        >
            {{ buttonText }}
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { NInput, NButton, NDropdown } from 'naive-ui'
import type { PlatformOption } from '../../config/platforms'
import AppIcon from '../common/AppIcon.vue'

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
    },
)

const emit = defineEmits<{
    (e: 'update:keyword', value: string): void
    (e: 'update:platform', value: string): void
    (e: 'search'): void
    (e: 'clear'): void
}>()

const keywordModel = ref(props.keyword)

const PLATFORM_COLORS: Record<string, string> = {
    qqmusic: '#17a37a',
    kuwo: '#f59e0b',
    kugou: '#3b82f6',
    netease: '#ef4444',
}

const platformDotColor = computed(
    () => PLATFORM_COLORS[props.platform] ?? 'var(--color-primary)',
)

// 当前平台对应的显示 label
const currentPlatformLabel = computed(() => {
    const found = props.platformOptions.find((p) => p.key === props.platform)
    return found ? found.label : props.platform
})

// 下拉选项格式：Naive UI 需要 { label, key } 结构
const platformDropdownOptions = computed(() => {
    return props.platformOptions.map((p) => ({
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
    },
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

<style scoped>
.search-bar {
    --search-height: 44px;
    --search-font-size: 14px;

    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    margin-bottom: 16px;
    background: var(--bg-elevated);
    padding: 6px 6px 6px 8px;
    border: 1px solid var(--border-color);
    border-radius: 999px;
    box-shadow: var(--shadow-sm);
    transition:
        border-color 0.2s ease,
        box-shadow 0.2s ease;
}

.search-bar:focus-within {
    border-color: var(--color-primary);
    box-shadow: 0 0 0 3px var(--color-primary-soft);
}

.platform-pill {
    height: 36px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--bg-surface-subtle);
    color: var(--color-text);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    transition:
        background-color 0.18s ease,
        border-color 0.18s ease;
}

.platform-pill:hover {
    border-color: var(--border-color-strong);
}

.platform-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
}

.platform-arrow {
    color: var(--color-text-secondary);
    font-size: 11px;
}

.search-leading-icon {
    display: inline-flex;
    align-items: center;
    color: var(--color-text-secondary);
    margin-left: 2px;
    flex-shrink: 0;
}

/* 输入框容器自动撑开 */
.search-bar .search-input {
    flex: 1;
    min-width: 0;
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
    height: var(--search-height) !important;
    --n-padding-left: 4px;
    --n-padding-right: 8px;
    border-radius: 0 !important;
}

.search-input :deep(.n-input-wrapper) {
    background: transparent !important;
    border: none !important;
    height: 100% !important;
}

.search-input :deep(.n-input__input) {
    padding: 0 !important;
    font-size: var(--search-font-size) !important;
    height: 100% !important;
    line-height: var(--search-height) !important;
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
    color: inherit !important;
}

.search-input :deep(.n-input__input-el) {
    height: 100%;
}

.search-input :deep(.n-input__input-el::placeholder) {
    color: var(--color-text-secondary) !important;
    opacity: 0.75;
}

.search-input :deep(.n-input__border),
.search-input :deep(.n-input__state-border) {
    display: none !important;
}

.search-input :deep(.n-input__suffix) {
    align-items: center;
}

.search-bar .search-btn {
    height: 38px !important;
    padding: 0 20px !important;
    font-size: 13px !important;
    font-weight: 600 !important;
    border-radius: 999px !important;
    display: inline-flex !important;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
}

@media (max-width: 520px) {
    .search-leading-icon {
        display: none;
    }

    .search-bar .search-btn {
        padding: 0 14px !important;
    }
}
</style>

<template>
    <section v-if="!loading && keywords.length > 0" class="hot-keywords">
        <div class="hot-header">
            <AppIcon name="flame" :size="16" class="hot-icon" />
            <span>热搜推荐</span>
        </div>
        <div class="hot-list">
            <button
                v-for="(word, index) in keywords"
                :key="word"
                type="button"
                class="hot-pill"
                @click="$emit('select', word)"
            >
                <span class="hot-rank" :class="{ 'is-top': index < 3 }">
                    {{ index + 1 }}
                </span>
                <span class="hot-word">{{ word }}</span>
            </button>
        </div>
    </section>
    <div v-else-if="loading" class="hot-loading">
        <n-spin size="small" />
    </div>
</template>

<script setup lang="ts">
import { NSpin } from 'naive-ui'
import AppIcon from '../common/AppIcon.vue'

defineProps<{
    keywords: string[]
    loading: boolean
}>()

defineEmits<{
    (e: 'select', word: string): void
}>()
</script>

<style scoped>
.hot-keywords {
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-card);
    padding: 16px 18px;
    box-shadow: var(--shadow-sm);
}

.hot-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
    margin-bottom: 12px;
}

.hot-icon {
    color: var(--color-primary);
}

.hot-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.hot-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    max-width: 100%;
    min-height: 34px;
    padding: 6px 13px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--bg-surface-subtle);
    color: var(--color-text);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    transition:
        border-color 0.18s ease,
        background-color 0.18s ease,
        transform 0.15s ease;
}

.hot-pill:hover {
    border-color: var(--color-primary);
    background: var(--color-primary-soft);
    transform: translateY(-1px);
}

.hot-rank {
    font-size: 11px;
    font-weight: 700;
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
}

.hot-rank.is-top {
    color: var(--color-primary);
}

.hot-word {
    min-width: 0;
    overflow-wrap: anywhere;
}

.hot-loading {
    display: flex;
    justify-content: center;
    padding: 20px 0;
}
</style>

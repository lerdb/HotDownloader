<template>
    <div class="load-more-wrapper">
        <n-button :loading="loading" :disabled="disabled" @click="$emit('click')">
            {{ loading ? '加载中...' : '加载更多' }}
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { NButton } from 'naive-ui'

defineProps<{
    loading: boolean
    disabled: boolean
}>()

defineEmits<{
    (e: 'click'): void
}>()
</script>

<style scoped>
.load-more-wrapper {
    display: flex;
    justify-content: center;
    margin-top: 16px;
}
</style>
<template>
    <n-form-item label="文件命名模板">
        <n-input :value="settingsStore.settings.namingTemplate"
            @update:value="(val) => (settingsStore.settings.namingTemplate = val)" placeholder="{song} - {artist}" />
        <template #feedback>
            <div class="template-help">
                可使用变量：<code>{song}</code>（歌名）、<code>{artist}</code>（歌手）、<code>{album}</code>（专辑）、<code>{quality}</code>（音质）<br />
                若替换后结果为空或仅含非法字符，将自动使用默认模板“歌名 - 歌手”
            </div>
        </template>
    </n-form-item>
</template>

<script setup lang="ts">
import { NFormItem, NInput } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'

const settingsStore = useSettingsStore()
</script>

<style scoped>
.template-help {
    font-size: 12px;
    color: var(--n-text-color-3);
    margin-top: 4px;
    line-height: 1.6;
}

code {
    background: rgba(127, 127, 127, 0.15);
    padding: 1px 4px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 12px;
    color: inherit;
}
</style>
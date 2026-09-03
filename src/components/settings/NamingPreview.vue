<template>
    <n-form-item label="示例">
        <div class="preview-container">
            <div class="preview-line">
                <span class="preview-label">歌手：</span>
                <code class="preview-value">{{ exampleArtists }}</code>
            </div>
            <div class="preview-line">
                <span class="preview-label">文件名：</span>
                <code class="preview-value">{{ exampleFilename }}.flac</code>
            </div>
        </div>
    </n-form-item>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { NFormItem } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'

const settingsStore = useSettingsStore()

// 用于预览的示例歌曲信息。artist 字段在示例中按用户配置的歌手分隔符拼接，
// 这样两个设置的修改可以即时在同一个示例里看到效果。
const exampleSong = {
    song: '千里之外',
    artistList: ['周杰伦', '费玉清'],
    album: '依然范特西',
    quality: 'flac',
}

// 过滤非法字符的函数（与后端 sanitize_name 一致）
const sanitize = (raw: string) => raw.replace(/[\\/:*?"<>|]/g, '_')

// 当前配置下的多歌手拼接结果（空值时回退为中文顿号）
const exampleArtists = computed(() => {
    const sep = settingsStore.settings.artistSeparator || '、'
    return exampleSong.artistList.join(sep)
})

// 根据当前模板与歌手拼接结果，生成示例文件名
const exampleFilename = computed(() => {
    const artist = exampleArtists.value
    const template = settingsStore.settings.namingTemplate || '{song} - {artist}'
    let name = template
        .replaceAll('{song}', exampleSong.song)
        .replaceAll('{artist}', artist)
        .replaceAll('{album}', exampleSong.album)
        .replaceAll('{quality}', exampleSong.quality)

    const sanitized = sanitize(name).trim()
    if (!sanitized) {
        const fallback = '{song} - {artist}'
            .replaceAll('{song}', exampleSong.song)
            .replaceAll('{artist}', artist)
            .replaceAll('{album}', exampleSong.album)
            .replaceAll('{quality}', exampleSong.quality)
        const fallbackSanitized = sanitize(fallback).trim()
        return fallbackSanitized || '未知歌曲'
    }
    return sanitized
})
</script>

<style scoped>

.preview-container {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.preview-line {
    display: block;
    /* 确保每行占满 */
    width: 100%;
    font-size: 12px;
    color: var(--n-text-color-3);
    line-height: 1.8;
    word-break: break-all;
}

.preview-label {
    color: var(--n-text-color-3);
    margin-right: 4px;
}

.preview-value {
    color: var(--n-text-color);
    font-family: monospace;
    font-size: 12px;
    background: rgba(127, 127, 127, 0.15);
    padding: 1px 4px;
    border-radius: 3px;
}
</style>

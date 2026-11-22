<template>
    <div class="playlist-home">
        <header class="page-header">
            <div>
                <h1>歌单中心</h1>
                <p>导入外部歌单链接或浏览已登录账号的个人歌单</p>
            </div>
        </header>

        <button
            v-if="!isNativeRuntime()"
            type="button"
            class="monitor-entry-card"
            @click="router.push('/playlist/monitors')"
        >
            <span class="monitor-entry-icon">
                <AppIcon name="radar" :size="20" />
            </span>
            <span class="monitor-entry-body">
                <strong>歌单监控与自动补齐</strong>
                <span>定时检查歌单新增曲目并与本地音乐库自动匹配</span>
            </span>
            <AppIcon
                name="chevron-right"
                :size="18"
                class="monitor-entry-arrow"
            />
        </button>

        <section class="playlist-section import-card">
            <div class="section-header">
                <AppIcon name="playlist" :size="18" class="section-icon" />
                <h2>导入歌单</h2>
            </div>
            <SearchBar
                v-model:keyword="importInput"
                v-model:platform="currentPlatform"
                :platform-options="PLATFORMS"
                placeholder="请输入歌单链接或 ID"
                button-text="导入歌单"
                @search="handleImport"
            />
        </section>

        <MyPlaylists
            platform="qqmusic"
            name="QQ 音乐"
            @open="(item) => emit('open-my', item)"
        />
        <MyPlaylists
            platform="kugou"
            name="酷狗音乐"
            @open="(item) => emit('open-my', item)"
        />
        <MyPlaylists
            platform="netease"
            name="网易云音乐"
            @open="(item) => emit('open-my', item)"
        />
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import SearchBar from '../search/SearchBar.vue'
import MyPlaylists from './MyPlaylists.vue'
import AppIcon from '../common/AppIcon.vue'
import { DEFAULT_PLATFORM, PLATFORMS } from '../../config/platforms'
import type { PlaylistSearchItem } from '../../types'
import { isNativeRuntime } from '../../api/runtimeApi'

const emit = defineEmits<{
    (e: 'import', platform: string, input: string): void
    (e: 'open-my', item: PlaylistSearchItem): void
}>()
const router = useRouter()
const currentPlatform = ref(DEFAULT_PLATFORM)
const importInput = ref('')
function handleImport() {
    const input = importInput.value.trim()
    if (input) emit('import', currentPlatform.value, input)
}
</script>

<style scoped>
.playlist-home {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
}

.page-header h1 {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin: 0 0 4px;
}

.page-header p {
    font-size: 13px;
    color: var(--color-text-secondary);
}

.monitor-entry-card {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px 18px;
    border-radius: 16px;
    border: 1px solid var(--border-color);
    background: var(--bg-sidebar);
    color: var(--color-text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    box-shadow: var(--shadow-sm);
    transition:
        border-color 0.18s ease,
        background-color 0.18s ease,
        transform 0.15s ease;
}

.monitor-entry-card:hover {
    border-color: var(--color-primary);
    background: var(--bg-elevated);
    transform: translateY(-1px);
}

.monitor-entry-icon {
    width: 42px;
    height: 42px;
    border-radius: 12px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    display: grid;
    place-items: center;
    flex-shrink: 0;
}

.monitor-entry-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
}

.monitor-entry-body strong {
    font-size: 15px;
    font-weight: 600;
}

.monitor-entry-body span {
    font-size: 12px;
    color: var(--color-text-secondary);
}

.monitor-entry-arrow {
    color: var(--color-text-secondary);
    flex-shrink: 0;
}

.import-card {
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 16px;
    padding: 16px 18px;
    box-shadow: var(--shadow-sm);
}

.playlist-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.section-header {
    display: flex;
    align-items: center;
    gap: 8px;
}

.section-icon {
    color: var(--color-primary);
}

.playlist-section h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
}

.playlist-home :deep(.search-bar) {
    margin-bottom: 0;
}

@media (max-width: 767px) {
    .page-header h1 {
        font-size: 22px;
    }
}
</style>

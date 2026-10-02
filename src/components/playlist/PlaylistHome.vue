<template>
    <div class="playlist-home">
        <n-button
            v-if="!isNativeRuntime()"
            @click="router.push('/playlist/monitors')"
            >歌单监控与自动补齐</n-button
        >
        <section class="playlist-section">
            <h2>导入歌单</h2>
            <SearchBar
                v-model:keyword="importInput"
                v-model:platform="currentPlatform"
                :platform-options="PLATFORMS"
                placeholder="请输入歌单链接或 ID"
                button-text="导入歌单"
                @search="handleImport"
            />
        </section>

        <section class="playlist-section">
            <h2>我的 QQ 音乐歌单</h2>
            <div v-if="myLoading" class="loading-wrapper">
                <n-spin size="medium" />
            </div>
            <n-alert v-else-if="myError" type="error" title="获取我的歌单失败">
                {{ myError }}
                <n-button @click="refreshMyPlaylists">重试</n-button>
            </n-alert>
            <div v-else-if="!loggedIn" class="empty-wrapper">
                <n-empty description="登录 QQ 音乐后即可查看自己创建的歌单">
                    <template #extra>
                        <n-button @click="router.push('/settings')"
                            >前往登录</n-button
                        >
                    </template>
                </n-empty>
            </div>
            <PlaylistSearchResult
                v-else
                :playlists="myPlaylists"
                :has-more="false"
                :loading-more="false"
                empty-description="暂无自己创建的歌单"
                @click-playlist="(item) => emit('open-my', item)"
            />
        </section>
    </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { NAlert, NButton, NEmpty, NSpin } from 'naive-ui'
import SearchBar from '../search/SearchBar.vue'
import PlaylistSearchResult from '../search/PlaylistSearchResult.vue'
import * as musicApi from '../../api/musicApi'
import { DEFAULT_PLATFORM, PLATFORMS } from '../../config/platforms'
import type { PlaylistSearchItem } from '../../types'
import { isNativeRuntime } from '../../api/runtimeApi'

const emit = defineEmits<{
    (e: 'import', platform: string, input: string): void
    (e: 'open-my', item: PlaylistSearchItem): void
}>()

const router = useRouter()
const route = useRoute()
const currentPlatform = ref(DEFAULT_PLATFORM)
const importInput = ref('')
const loggedIn = ref(false)
const myPlaylists = ref<PlaylistSearchItem[]>([])
const myLoading = ref(false)
const myError = ref('')
let requestId = 0

function handleImport() {
    const input = importInput.value.trim()
    if (input) emit('import', currentPlatform.value, input)
}

async function refreshMyPlaylists() {
    // 登录可能在设置页发生变化；每次进入入口页都重新读取当前账号。
    const currentRequest = ++requestId
    myLoading.value = true
    myError.value = ''

    try {
        const status = await musicApi.getLoginStatus('qqmusic')
        if (currentRequest !== requestId) return

        loggedIn.value = status.logged_in
        if (!status.logged_in) {
            myPlaylists.value = []
            return
        }

        const result = await musicApi.fetchCreatedPlaylists()
        if (currentRequest === requestId) {
            myPlaylists.value = result.playlists
        }
    } catch (error) {
        if (currentRequest === requestId) {
            myError.value =
                error instanceof Error ? error.message : String(error)
        }
    } finally {
        if (currentRequest === requestId) {
            myLoading.value = false
        }
    }
}

onMounted(() => {
    void refreshMyPlaylists()
})

// 页面被 keep-alive 缓存；从设置页返回时重新读取可能变化的 QQ 登录态。
watch(
    () => route.path,
    (path, previousPath) => {
        if (path === '/playlist' && previousPath !== '/playlist') {
            void refreshMyPlaylists()
        }
    },
)

onUnmounted(() => {
    requestId++
})
</script>

<style scoped>
.playlist-home {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
}

.playlist-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.playlist-section h2 {
    margin: 0;
    font-size: 18px;
}

/* SearchBar 自带下边距；入口页使用父容器的 gap 控制间距。 */
.playlist-home :deep(.search-bar) {
    margin-bottom: 0;
}

.loading-wrapper,
.empty-wrapper {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}
</style>

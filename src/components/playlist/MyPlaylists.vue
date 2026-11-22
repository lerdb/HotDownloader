<template>
    <section class="my-playlists">
        <div class="section-header">
            <span
                class="platform-dot"
                :style="{ backgroundColor: dotColor }"
                aria-hidden="true"
            />
            <h2>我的{{ name }}歌单</h2>
        </div>

        <div v-if="loading" class="placeholder-card">
            <n-spin size="medium" />
        </div>
        <n-alert v-else-if="error" type="error" title="获取我的歌单失败">
            {{ error }}
            <n-button size="small" @click="refresh">重试</n-button>
        </n-alert>
        <div v-else-if="!loggedIn" class="placeholder-card">
            <n-empty :description="`登录${name}后即可查看账号歌单`">
                <template #extra>
                    <n-button
                        round
                        secondary
                        type="primary"
                        @click="
                            router.push({
                                path: '/settings',
                                query: { platform },
                            })
                        "
                    >
                        前往登录
                    </n-button>
                </template>
            </n-empty>
        </div>
        <PlaylistSearchResult
            v-else
            :playlists="playlists"
            :has-more="false"
            :loading-more="false"
            empty-description="暂无歌单"
            @click-playlist="(item) => emit('open', item)"
        />
    </section>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { NAlert, NButton, NEmpty, NSpin } from 'naive-ui'
import PlaylistSearchResult from '../search/PlaylistSearchResult.vue'
import * as musicApi from '../../api/musicApi'
import { isLoginPlatform } from '../../api/platformLoginApi'
import { useAccountStore } from '../../stores/accountStore'
import type { PlaylistSearchItem } from '../../types'

const props = defineProps<{ platform: string; name: string }>()
const emit = defineEmits<{ (e: 'open', item: PlaylistSearchItem): void }>()
const router = useRouter()
const route = useRoute()
const accounts = useAccountStore()
const loggedIn = ref(false)
const playlists = ref<PlaylistSearchItem[]>([])
const loading = ref(false)
const error = ref('')
let requestId = 0

const dotColor = computed(
    () =>
        ({
            qqmusic: '#17a37a',
            kugou: '#3b82f6',
            netease: '#ef4444',
        })[props.platform] ?? 'var(--color-primary)',
)

async function refresh() {
    const current = ++requestId
    loading.value = true
    error.value = ''
    try {
        if (!isLoginPlatform(props.platform)) {
            loggedIn.value = false
            playlists.value = []
            return
        }
        const session = accounts.sessions[props.platform]
        await session.whenIdle()
        if (current !== requestId) return
        await session.checkStatus()
        if (!session.statusKnown)
            throw new Error(session.error || '登录状态检查失败')
        if (current !== requestId) return
        loggedIn.value = session.loggedIn
        if (!loggedIn.value) {
            playlists.value = []
            return
        }
        const result = await musicApi.fetchCreatedPlaylists(props.platform)
        if (current === requestId) playlists.value = result.playlists
    } catch (err) {
        if (current === requestId)
            error.value = err instanceof Error ? err.message : String(err)
    } finally {
        if (current === requestId) loading.value = false
    }
}
onMounted(() => void refresh())
watch(
    () => route.path,
    (path, previous) => {
        if (path === '/playlist' && path !== previous) void refresh()
    },
)
onUnmounted(() => requestId++)
</script>

<style scoped>
.my-playlists {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
}

.section-header {
    display: flex;
    align-items: center;
    gap: 8px;
}

.platform-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
}

h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
}

.placeholder-card {
    display: flex;
    justify-content: center;
    padding: 32px 16px;
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 16px;
}
</style>
